//! Do evento cru ao passo pronto.
//!
//! Duas peças:
//!
//! - [`Grouper`], uma máquina de estados pura que decide *quando* capturar e
//!   *o que* virou passo — é ela que junta oito teclas em "Digitou `nota.pdf`"
//!   e que promove dois cliques seguidos a um duplo clique. Não toca em tela
//!   nem em thread, então é testável sozinha.
//! - [`Recorder`], a thread que executa as decisões do `Grouper`: tira o
//!   screenshot, consulta a acessibilidade, comprime PNG e miniatura.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use crossbeam_channel::{Receiver, Sender};
use stepeasy_core::bundle::{image_path, thumb_path};
use stepeasy_core::geometry::Point;
use stepeasy_core::model::{ImageRef, MouseButton, ScrollDirection, Step, StepKind};
use stepeasy_core::scope::CaptureScope;
use stepeasy_core::{caption, render};

use crate::event::RawEvent;
use crate::{Platform, Result};

/// Silêncio que fecha um grupo de digitação ou de rolagem.
pub const IDLE_FLUSH: Duration = Duration::from_millis(800);
/// Janela de tempo para dois cliques virarem um duplo clique.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// Deslocamento a partir do qual pressionar-e-soltar vira arrasto, em pixels.
pub const DRAG_THRESHOLD: i32 = 8;
/// Lado maior da miniatura da timeline.
const THUMB_SIDE: u32 = 320;
const THUMB_QUALITY: u8 = 78;

/// O que o [`Grouper`] manda a thread de captura fazer.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Tire o screenshot agora e guarde — o passo correspondente vem a seguir.
    Capture { at: Point },
    /// Monte um passo com o frame guardado.
    Emit(Pending),
    /// O passo anterior era um clique simples e virou duplo clique. Descarte o
    /// frame guardado e corrija o passo já emitido.
    UpgradePrevious { kind: StepKind },
}

/// Passo decidido pelo agrupador, ainda sem imagem.
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub kind: StepKind,
    pub at: Point,
    pub time: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Group {
    None,
    Typing,
    Scrolling { direction: ScrollDirection },
}

/// Máquina de estados que transforma eventos crus em passos.
#[derive(Debug)]
pub struct Grouper {
    group: Group,
    buffer: String,
    scroll_amount: i32,
    group_start: Option<Pending>,
    last_event: Option<DateTime<Utc>>,
    /// Estado do botão pressionado, para distinguir clique de arrasto.
    down: Option<(MouseButton, Point, DateTime<Utc>)>,
    /// Último clique emitido, para detectar duplo clique.
    last_click: Option<(MouseButton, Point, DateTime<Utc>)>,
}

impl Default for Grouper {
    fn default() -> Self {
        Self::new()
    }
}

impl Grouper {
    pub fn new() -> Self {
        Self {
            group: Group::None,
            buffer: String::new(),
            scroll_amount: 0,
            group_start: None,
            last_event: None,
            down: None,
            last_click: None,
        }
    }

    /// Processa um evento e devolve as ações em ordem.
    pub fn push(&mut self, event: &RawEvent) -> Vec<Action> {
        let mut actions = Vec::new();
        self.last_event = Some(event.time());

        match event {
            RawEvent::MouseDown { button, at, time } => {
                self.flush_into(&mut actions);
                self.down = Some((*button, *at, *time));
                actions.push(Action::Capture { at: *at });
            }

            // O instante que interessa é o do *press*, guardado em `self.down`:
            // é ele que ancora o passo e a janela do duplo clique.
            RawEvent::MouseUp { button, at, time: _ } => {
                let Some((down_button, down_at, down_time)) = self.down.take() else {
                    return actions;
                };
                if down_button != *button {
                    return actions;
                }

                let moveu = (down_at.x - at.x).abs() > DRAG_THRESHOLD
                    || (down_at.y - at.y).abs() > DRAG_THRESHOLD;

                if moveu {
                    self.last_click = None;
                    actions.push(Action::Emit(Pending {
                        kind: StepKind::Drag {
                            button: *button,
                            to: *at,
                        },
                        at: down_at,
                        time: down_time,
                    }));
                    return actions;
                }

                if self.is_double_click(*button, down_at, down_time) {
                    self.last_click = None;
                    actions.push(Action::UpgradePrevious {
                        kind: StepKind::DoubleClick { button: *button },
                    });
                    return actions;
                }

                self.last_click = Some((*button, down_at, down_time));
                actions.push(Action::Emit(Pending {
                    kind: StepKind::Click { button: *button },
                    at: down_at,
                    time: down_time,
                }));
            }

            RawEvent::Wheel {
                delta,
                horizontal,
                at,
                time,
            } => {
                let direction = match (horizontal, delta.is_positive()) {
                    (false, true) => ScrollDirection::Up,
                    (false, false) => ScrollDirection::Down,
                    (true, true) => ScrollDirection::Right,
                    (true, false) => ScrollDirection::Left,
                };

                match self.group {
                    Group::Scrolling { direction: atual } if atual == direction => {
                        self.scroll_amount += delta.abs();
                        return actions;
                    }
                    _ => self.flush_into(&mut actions),
                }

                self.group = Group::Scrolling { direction };
                self.scroll_amount = delta.abs();
                self.group_start = Some(Pending {
                    kind: StepKind::Scroll {
                        direction,
                        amount: 0,
                    },
                    at: *at,
                    time: *time,
                });
                actions.push(Action::Capture { at: *at });
            }

            RawEvent::Key {
                text,
                combo,
                with_modifier,
                at,
                time,
                ..
            } => {
                let digitavel = !*with_modifier && text.is_some();

                if digitavel {
                    let ch = text.as_deref().unwrap_or_default();
                    if self.group != Group::Typing {
                        self.flush_into(&mut actions);
                        self.group = Group::Typing;
                        self.buffer.clear();
                        self.group_start = Some(Pending {
                            kind: StepKind::Type {
                                text: String::new(),
                            },
                            at: *at,
                            time: *time,
                        });
                        actions.push(Action::Capture { at: *at });
                    }
                    self.buffer.push_str(ch);
                    return actions;
                }

                self.flush_into(&mut actions);
                self.last_click = None;
                actions.push(Action::Capture { at: *at });
                actions.push(Action::Emit(Pending {
                    kind: StepKind::Key {
                        combo: combo.clone(),
                    },
                    at: *at,
                    time: *time,
                }));
            }
        }

        actions
    }

    /// Fecha grupos abertos que já passaram do tempo de silêncio.
    ///
    /// A thread de captura chama isto periodicamente; sem ele, a última palavra
    /// digitada só sairia quando o usuário fizesse outra coisa.
    pub fn tick(&mut self, now: DateTime<Utc>) -> Vec<Action> {
        let mut actions = Vec::new();
        let Some(last) = self.last_event else {
            return actions;
        };
        if self.group == Group::None {
            return actions;
        }
        let idle = now.signed_duration_since(last).to_std().unwrap_or_default();
        if idle >= IDLE_FLUSH {
            self.flush_into(&mut actions);
        }
        actions
    }

    /// Fecha tudo o que estiver aberto — usado ao parar a gravação.
    pub fn finish(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        self.flush_into(&mut actions);
        self.down = None;
        actions
    }

    fn is_double_click(&self, button: MouseButton, at: Point, time: DateTime<Utc>) -> bool {
        let Some((prev_button, prev_at, prev_time)) = self.last_click else {
            return false;
        };
        prev_button == button
            && (prev_at.x - at.x).abs() <= DRAG_THRESHOLD
            && (prev_at.y - at.y).abs() <= DRAG_THRESHOLD
            && time
                .signed_duration_since(prev_time)
                .to_std()
                .map(|d| d <= DOUBLE_CLICK)
                .unwrap_or(false)
    }

    fn flush_into(&mut self, actions: &mut Vec<Action>) {
        let group = std::mem::replace(&mut self.group, Group::None);
        let Some(start) = self.group_start.take() else {
            return;
        };

        match group {
            Group::Typing if !self.buffer.is_empty() => {
                actions.push(Action::Emit(Pending {
                    kind: StepKind::Type {
                        text: std::mem::take(&mut self.buffer),
                    },
                    ..start
                }));
            }
            Group::Scrolling { direction } => {
                actions.push(Action::Emit(Pending {
                    kind: StepKind::Scroll {
                        direction,
                        amount: self.scroll_amount,
                    },
                    ..start
                }));
                self.scroll_amount = 0;
            }
            _ => {
                self.buffer.clear();
            }
        }
    }
}

/// Um passo pronto, com os binários que vão para o pacote.
pub struct CapturedStep {
    pub step: Step,
    pub png: Option<Vec<u8>>,
    pub thumb: Option<Vec<u8>>,
}

/// Mensagens da thread de captura para a UI.
pub enum RecorderMessage {
    Step(Box<CapturedStep>),
    /// O passo anterior virou duplo clique.
    UpgradeLast { kind: StepKind, caption: String },
    /// O usuário pressionou o atalho de parada.
    StopRequested,
    /// A gravação passou a pausada (`true`) ou voltou a gravar (`false`).
    Paused(bool),
    Error(String),
    Stopped,
}

/// Como iniciar uma gravação.
#[derive(Debug, Clone)]
pub struct RecorderConfig {
    pub scope: CaptureScope,
    /// Combinação que encerra a gravação.
    pub stop_combo: String,
    /// Combinação que alterna entre pausado e gravando.
    pub pause_combo: String,
    /// Primeiro número usado para nomear as imagens.
    ///
    /// Ao continuar uma gravação isto vem de
    /// [`Recording::next_image_index`](stepeasy_core::Recording::next_image_index),
    /// senão as capturas novas sobrescreveriam as antigas dentro do pacote.
    pub first_image_index: u32,
}

impl RecorderConfig {
    pub fn new(scope: CaptureScope, stop_combo: impl Into<String>, pause_combo: impl Into<String>) -> Self {
        Self {
            scope,
            stop_combo: stop_combo.into(),
            pause_combo: pause_combo.into(),
            first_image_index: 1,
        }
    }

    pub fn starting_at(mut self, first_image_index: u32) -> Self {
        self.first_image_index = first_image_index.max(1);
        self
    }
}

/// Gravação em andamento.
pub struct Recorder {
    running: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    input_stop: Option<Box<dyn FnOnce() + Send>>,
    pub messages: Receiver<RecorderMessage>,
}

impl Recorder {
    /// Instala os ganchos e inicia a thread que monta os passos.
    ///
    /// Os atalhos de parar e pausar são filtrados do fluxo de eventos, então
    /// não viram passo — e, por serem tratados aqui e não pela janela,
    /// funcionam com o aplicativo minimizado.
    pub fn start(mut platform: Platform, config: RecorderConfig) -> Result<Self> {
        let (raw_tx, raw_rx) = crossbeam_channel::unbounded::<RawEvent>();
        let (msg_tx, msg_rx) = crossbeam_channel::unbounded::<RecorderMessage>();

        platform.input.start(raw_tx)?;

        let running = Arc::new(AtomicBool::new(true));
        let stop_requested = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));

        let Platform {
            mut input,
            screen,
            probe,
        } = platform;

        let ctx = WorkerContext {
            running: running.clone(),
            stop_requested: stop_requested.clone(),
            paused: paused.clone(),
            config,
            screen,
            probe,
        };

        std::thread::Builder::new()
            .name("stepeasy-worker".into())
            .spawn(move || worker(ctx, raw_rx, msg_tx))
            .map_err(|e| crate::Error::Hook(e.to_string()))?;

        Ok(Self {
            running,
            stop_requested,
            paused,
            input_stop: Some(Box::new(move || input.stop())),
            messages: msg_rx,
        })
    }

    /// `true` depois que o usuário pressiona o atalho de parada.
    pub fn stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::SeqCst)
    }

    /// `true` enquanto a gravação está pausada.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Alterna pausado/gravando a partir da interface (o atalho faz o mesmo
    /// caminho, mas de dentro da thread de captura).
    pub fn toggle_pause(&self) {
        self.paused.fetch_xor(true, Ordering::SeqCst);
    }

    /// Para os ganchos e a thread. Os passos já emitidos continuam no canal.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(stop) = self.input_stop.take() {
            stop();
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
    }
}

struct WorkerContext {
    running: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    config: RecorderConfig,
    screen: Box<dyn crate::ScreenGrabber>,
    probe: Box<dyn crate::UiProbe>,
}

fn worker(ctx: WorkerContext, raw_rx: Receiver<RawEvent>, msg_tx: Sender<RecorderMessage>) {
    let WorkerContext {
        running,
        stop_requested,
        paused,
        config,
        screen,
        probe,
    } = ctx;
    let RecorderConfig {
        scope,
        stop_combo,
        pause_combo,
        first_image_index,
    } = config;

    let mut grouper = Grouper::new();
    let mut held: Option<(crate::Frame, Option<stepeasy_core::UiTarget>)> = None;
    // O contador aponta para o próximo número a usar; `Emit` incrementa antes
    // de nomear, então ele começa um abaixo do primeiro índice livre.
    let mut counter: u32 = first_image_index.saturating_sub(1);

    let apply = |actions: Vec<Action>,
                 held: &mut Option<(crate::Frame, Option<stepeasy_core::UiTarget>)>,
                 counter: &mut u32| {
        for action in actions {
            match action {
                Action::Capture { at } => {
                    // A consulta de acessibilidade vem antes do screenshot: a UI
                    // ainda está no estado em que o usuário clicou.
                    let target = probe.element_at(at);
                    match screen.grab(&scope, at) {
                        Ok(frame) => *held = Some((frame, target)),
                        Err(err) => {
                            let _ = msg_tx.send(RecorderMessage::Error(err.to_string()));
                            *held = None;
                        }
                    }
                }
                Action::Emit(pending) => {
                    *counter += 1;
                    match build_step(pending, held.take(), *counter) {
                        Ok(captured) => {
                            let _ = msg_tx.send(RecorderMessage::Step(Box::new(captured)));
                        }
                        Err(err) => {
                            let _ = msg_tx.send(RecorderMessage::Error(err.to_string()));
                        }
                    }
                }
                Action::UpgradePrevious { kind } => {
                    *held = None;
                    let mut fake = Step::new(kind.clone());
                    caption::refresh(&mut fake);
                    let _ = msg_tx.send(RecorderMessage::UpgradeLast {
                        kind,
                        caption: fake.caption,
                    });
                }
            }
        }
    };

    while running.load(Ordering::SeqCst) {
        match raw_rx.recv_timeout(Duration::from_millis(120)) {
            Ok(event) => {
                if combo_igual(&event, &stop_combo) {
                    stop_requested.store(true, Ordering::SeqCst);
                    let _ = msg_tx.send(RecorderMessage::StopRequested);
                    break;
                }

                if combo_igual(&event, &pause_combo) {
                    let agora_pausado = !paused.fetch_xor(true, Ordering::SeqCst);
                    if agora_pausado {
                        // Fecha o que estiver aberto: a palavra digitada até
                        // aqui vira passo agora, e não colada no que o usuário
                        // digitar depois de retomar.
                        apply(grouper.finish(), &mut held, &mut counter);
                    }
                    let _ = msg_tx.send(RecorderMessage::Paused(agora_pausado));
                    continue;
                }

                if paused.load(Ordering::SeqCst) {
                    continue;
                }

                let actions = grouper.push(&event);
                apply(actions, &mut held, &mut counter);
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if paused.load(Ordering::SeqCst) {
                    continue;
                }
                let actions = grouper.tick(Utc::now());
                apply(actions, &mut held, &mut counter);
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }
    }

    let actions = grouper.finish();
    apply(actions, &mut held, &mut counter);
    let _ = msg_tx.send(RecorderMessage::Stopped);
}

/// `true` quando o evento é o atalho indicado.
///
/// Além do nome bater, o evento precisa ter modificador ou ser uma tecla que
/// não produz texto. Sem essa condição, um atalho configurado como uma letra
/// solta faria a digitação normal do usuário encerrar a gravação.
fn combo_igual(event: &RawEvent, alvo: &str) -> bool {
    if alvo.is_empty() {
        return false;
    }
    match event {
        RawEvent::Key {
            combo,
            with_modifier,
            text,
            ..
        } => combo.eq_ignore_ascii_case(alvo) && (*with_modifier || text.is_none()),
        _ => false,
    }
}

/// Monta o passo final: legenda, PNG e miniatura.
fn build_step(
    pending: Pending,
    frame: Option<(crate::Frame, Option<stepeasy_core::UiTarget>)>,
    counter: u32,
) -> Result<CapturedStep> {
    let mut step = Step::new(pending.kind);
    step.timestamp = pending.time;
    step.cursor = Some(pending.at);

    let mut png = None;
    let mut thumb = None;

    if let Some((frame, target)) = frame {
        step.target = target;
        step.scope_fallback = frame.fallback;

        // Sem acessibilidade, ao menos o título da janela capturada ajuda.
        if let Some(title) = frame.window_title.clone() {
            let entry = step.target.get_or_insert_with(Default::default);
            if entry.window_title.is_none() {
                entry.window_title = Some(title);
            }
        }

        let bytes = render::encode_png(&frame.image)?;
        thumb = render::thumbnail(&bytes, THUMB_SIDE, THUMB_QUALITY).ok();
        step.image = Some(ImageRef {
            path: image_path(counter),
            thumb_path: thumb.is_some().then(|| thumb_path(counter)),
            width: frame.image.width(),
            height: frame.image.height(),
            source_rect: frame.rect,
        });
        png = Some(bytes);
    }

    caption::refresh(&mut step);
    Ok(CapturedStep { step, png, thumb })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(1_700_000_000_000 + ms).unwrap()
    }

    fn tecla(c: &str, ms: i64) -> RawEvent {
        RawEvent::Key {
            vk: 0,
            text: Some(c.to_string()),
            combo: c.to_string(),
            with_modifier: false,
            at: Point::new(10, 10),
            time: t(ms),
        }
    }

    fn atalho(combo: &str, ms: i64) -> RawEvent {
        RawEvent::Key {
            vk: 0,
            text: None,
            combo: combo.to_string(),
            with_modifier: true,
            at: Point::new(10, 10),
            time: t(ms),
        }
    }

    fn down(x: i32, y: i32, ms: i64) -> RawEvent {
        RawEvent::MouseDown {
            button: MouseButton::Left,
            at: Point::new(x, y),
            time: t(ms),
        }
    }

    fn up(x: i32, y: i32, ms: i64) -> RawEvent {
        RawEvent::MouseUp {
            button: MouseButton::Left,
            at: Point::new(x, y),
            time: t(ms),
        }
    }

    fn emitidos(actions: &[Action]) -> Vec<StepKind> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Emit(p) => Some(p.kind.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn digitacao_vira_um_passo_so() {
        let mut g = Grouper::new();
        let mut acoes = Vec::new();
        for (i, c) in "nota".chars().enumerate() {
            acoes.extend(g.push(&tecla(&c.to_string(), i as i64 * 100)));
        }
        // Só um Capture, no começo da digitação, e nada emitido ainda.
        assert_eq!(
            acoes.iter().filter(|a| matches!(a, Action::Capture { .. })).count(),
            1
        );
        assert!(emitidos(&acoes).is_empty());

        let acoes = g.tick(t(1_500));
        assert_eq!(
            emitidos(&acoes),
            vec![StepKind::Type {
                text: "nota".into()
            }]
        );
    }

    #[test]
    fn digitacao_e_fechada_por_atalho_e_o_atalho_vira_passo() {
        let mut g = Grouper::new();
        g.push(&tecla("o", 0));
        g.push(&tecla("i", 50));
        let acoes = g.push(&atalho("Ctrl+S", 200));

        assert_eq!(
            emitidos(&acoes),
            vec![
                StepKind::Type { text: "oi".into() },
                StepKind::Key {
                    combo: "Ctrl+S".into()
                }
            ]
        );
        // O screenshot do atalho é tirado depois de fechar a digitação.
        let ordem: Vec<_> = acoes
            .iter()
            .map(|a| matches!(a, Action::Capture { .. }))
            .collect();
        assert_eq!(ordem, vec![false, true, false]);
    }

    #[test]
    fn clique_captura_no_press_e_emite_no_release() {
        let mut g = Grouper::new();
        let acoes = g.push(&down(100, 100, 0));
        assert_eq!(acoes, vec![Action::Capture { at: Point::new(100, 100) }]);

        let acoes = g.push(&up(102, 101, 90));
        assert_eq!(
            emitidos(&acoes),
            vec![StepKind::Click {
                button: MouseButton::Left
            }]
        );
        // O passo fica ancorado na posição e no instante do *press*.
        match &acoes[0] {
            Action::Emit(p) => {
                assert_eq!(p.at, Point::new(100, 100));
                assert_eq!(p.time, t(0));
            }
            other => panic!("esperava Emit, veio {other:?}"),
        }
    }

    #[test]
    fn dois_cliques_seguidos_viram_duplo_clique() {
        let mut g = Grouper::new();
        g.push(&down(50, 50, 0));
        g.push(&up(50, 50, 60));
        g.push(&down(51, 50, 200));
        let acoes = g.push(&up(51, 50, 240));

        assert_eq!(
            acoes,
            vec![Action::UpgradePrevious {
                kind: StepKind::DoubleClick {
                    button: MouseButton::Left
                }
            }]
        );
    }

    #[test]
    fn cliques_distantes_no_tempo_nao_viram_duplo() {
        let mut g = Grouper::new();
        g.push(&down(50, 50, 0));
        g.push(&up(50, 50, 60));
        g.push(&down(50, 50, 2_000));
        let acoes = g.push(&up(50, 50, 2_060));

        assert_eq!(
            emitidos(&acoes),
            vec![StepKind::Click {
                button: MouseButton::Left
            }]
        );
    }

    #[test]
    fn arrastar_vira_passo_de_arrasto() {
        let mut g = Grouper::new();
        g.push(&down(10, 10, 0));
        let acoes = g.push(&up(300, 200, 400));

        assert_eq!(
            emitidos(&acoes),
            vec![StepKind::Drag {
                button: MouseButton::Left,
                to: Point::new(300, 200)
            }]
        );
    }

    #[test]
    fn rolagem_na_mesma_direcao_e_agrupada() {
        let mut g = Grouper::new();
        let mut acoes = Vec::new();
        for i in 0..5 {
            acoes.extend(g.push(&RawEvent::Wheel {
                delta: -120,
                horizontal: false,
                at: Point::new(400, 400),
                time: t(i * 80),
            }));
        }
        assert_eq!(
            acoes.iter().filter(|a| matches!(a, Action::Capture { .. })).count(),
            1
        );

        let acoes = g.finish();
        assert_eq!(
            emitidos(&acoes),
            vec![StepKind::Scroll {
                direction: ScrollDirection::Down,
                amount: 600
            }]
        );
    }

    #[test]
    fn mudar_a_direcao_da_rolagem_fecha_o_grupo() {
        let mut g = Grouper::new();
        g.push(&RawEvent::Wheel {
            delta: -120,
            horizontal: false,
            at: Point::new(0, 0),
            time: t(0),
        });
        let acoes = g.push(&RawEvent::Wheel {
            delta: 120,
            horizontal: false,
            at: Point::new(0, 0),
            time: t(100),
        });
        assert_eq!(
            emitidos(&acoes),
            vec![StepKind::Scroll {
                direction: ScrollDirection::Down,
                amount: 120
            }]
        );
    }

    #[test]
    fn clique_fecha_digitacao_pendente() {
        let mut g = Grouper::new();
        g.push(&tecla("a", 0));
        let acoes = g.push(&down(10, 10, 300));
        assert_eq!(
            emitidos(&acoes),
            vec![StepKind::Type { text: "a".into() }]
        );
    }

    #[test]
    fn atalhos_sao_reconhecidos_sem_diferenciar_caixa() {
        let evento = atalho("Ctrl+Shift+F9", 0);
        assert!(combo_igual(&evento, "ctrl+shift+f9"));
        assert!(!combo_igual(&evento, "Ctrl+Shift+F10"));
        // Atalho vazio nunca casa, senão qualquer tecla pararia a gravação.
        assert!(!combo_igual(&evento, ""));

        // Uma letra digitada não dispara atalho, mesmo que alguém configure o
        // atalho como essa letra.
        assert!(!combo_igual(&tecla("a", 0), "A"));

        // Já uma tecla que não produz texto pode ser atalho sozinha.
        let f9 = RawEvent::Key {
            vk: 0,
            text: None,
            combo: "F9".into(),
            with_modifier: false,
            at: Point::new(0, 0),
            time: t(0),
        };
        assert!(combo_igual(&f9, "F9"));
    }

    #[test]
    fn config_nunca_comeca_a_numeracao_no_zero() {
        let config = RecorderConfig::new(CaptureScope::default(), "Ctrl+F9", "Ctrl+F10");
        assert_eq!(config.first_image_index, 1);
        assert_eq!(config.clone().starting_at(0).first_image_index, 1);
        assert_eq!(config.starting_at(12).first_image_index, 12);
    }

    #[test]
    fn tick_sem_grupo_aberto_nao_faz_nada() {
        let mut g = Grouper::new();
        assert!(g.tick(t(10_000)).is_empty());
        g.push(&down(1, 1, 0));
        assert!(g.tick(t(10_000)).is_empty());
    }
}
