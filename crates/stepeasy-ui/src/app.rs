//! Estado global da janela e a cola entre gravação, edição e arquivo.

use std::time::Duration;

use egui::{Key, KeyboardShortcut, Modifiers};
use stepeasy_capture::session::{Recorder, RecorderConfig, RecorderMessage};
use stepeasy_core::edit::History;
use stepeasy_core::export::{self, Format};
use stepeasy_core::scope::{CaptureScope, MonitorInfo};
use stepeasy_core::{caption, Project, Recording};
use uuid::Uuid;

use crate::textures::Textures;
use crate::toast::Toasts;
use crate::{screens, theme};

/// Combinação que encerra a gravação mesmo com a janela minimizada. É tratada
/// dentro do próprio fluxo de eventos capturados, então funciona sem depender
/// de um registrador de atalho global.
pub const STOP_HOTKEY: &str = "Ctrl+Shift+F9";
/// Alterna entre pausado e gravando, pelas mesmas razões do atalho de parada.
pub const PAUSE_HOTKEY: &str = "Ctrl+Shift+F10";

const KEY_SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::S);
const KEY_OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::O);
const KEY_UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::Z);
const KEY_REDO: KeyboardShortcut = KeyboardShortcut::new(
    Modifiers {
        command: true,
        shift: true,
        ..Modifiers::NONE
    },
    Key::Z,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Recorder,
    Editor,
}

pub struct App {
    pub screen: Screen,
    pub dark: bool,

    /// Projeto aberto. Existe assim que uma gravação começa ou um arquivo é
    /// aberto; o editor fica indisponível enquanto for `None`.
    pub project: Option<Project>,
    pub history: History,
    pub textures: Textures,
    pub toasts: Toasts,

    /// Passos selecionados na timeline, na ordem em que foram clicados.
    pub selection: Vec<Uuid>,
    /// Passo mostrado no painel central.
    pub focused: Option<Uuid>,

    /// Ferramenta de anotação ativa e o que está sendo desenhado.
    pub annot: crate::annotate::Estado,

    pub scope: CaptureScope,
    pub monitors: Vec<MonitorInfo>,
    pub minimize_while_recording: bool,
    pub recorder: Option<Recorder>,
    pub recorded_steps: usize,
    /// `true` quando a gravação atual está acrescentando a uma existente.
    pub continuing: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let dark = cc
            .storage
            .and_then(|s| s.get_string("dark"))
            .map(|v| v == "1")
            .unwrap_or_else(|| cc.egui_ctx.theme() == egui::Theme::Dark);

        let scope = cc
            .storage
            .and_then(|s| s.get_string("scope"))
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();

        theme::apply(&cc.egui_ctx, dark);

        let monitors = stepeasy_capture::platform()
            .screen
            .monitors()
            .unwrap_or_else(|err| {
                tracing::warn!("não foi possível listar os monitores: {err}");
                Vec::new()
            });

        Self {
            screen: Screen::Recorder,
            dark,
            project: None,
            history: History::new(),
            textures: Textures::default(),
            toasts: Toasts::default(),
            selection: Vec::new(),
            focused: None,
            annot: Default::default(),
            scope,
            monitors,
            minimize_while_recording: true,
            recorder: None,
            recorded_steps: 0,
            continuing: false,
        }
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    // ----- gravação -------------------------------------------------------

    /// `true` quando há uma gravação aberta à qual dá para acrescentar passos.
    pub fn can_continue_recording(&self) -> bool {
        self.project.is_some() && stepeasy_capture::is_supported() && !self.is_recording()
    }

    /// Começa uma gravação nova, descartando a que estiver aberta.
    pub fn start_recording(&mut self, ctx: &egui::Context) {
        self.begin_recording(ctx, false);
    }

    /// Acrescenta passos ao fim da gravação já aberta.
    pub fn continue_recording(&mut self, ctx: &egui::Context) {
        if self.project.is_none() {
            self.start_recording(ctx);
            return;
        }
        self.begin_recording(ctx, true);
    }

    fn begin_recording(&mut self, ctx: &egui::Context, continuar: bool) {
        if self.is_recording() {
            return;
        }
        if !stepeasy_capture::is_supported() {
            self.toasts
                .error("a gravação ainda não está disponível neste sistema operacional");
            return;
        }
        if !continuar {
            if let Some(project) = &self.project {
                if project.is_dirty() {
                    self.toasts.info(
                        "há alterações não salvas; elas serão substituídas pela nova gravação",
                    );
                }
            }
        }

        // Continuar reaproveita a numeração das imagens de onde ela parou;
        // recomeçar do 1 sobrescreveria as capturas que já estão no pacote.
        let primeira_imagem = if continuar {
            self.project
                .as_ref()
                .map_or(1, |p| p.recording.next_image_index())
        } else {
            1
        };

        let config = RecorderConfig::new(self.scope.clone(), STOP_HOTKEY, PAUSE_HOTKEY)
            .starting_at(primeira_imagem);

        match Recorder::start(stepeasy_capture::platform(), config) {
            Ok(recorder) => {
                if continuar {
                    // Um instantâneo vazio antes de acrescentar: assim um
                    // Ctrl+Z depois devolve a gravação como ela estava.
                    if let Some(project) = &mut self.project {
                        self.history
                            .edit(&mut project.recording, "Continuar gravação", |_| {});
                    }
                } else {
                    self.project = Some(Project::new(Recording::new(
                        nome_padrao(),
                        self.scope.clone(),
                    )));
                    self.history.clear();
                    self.textures.clear();
                    self.selection.clear();
                    self.focused = None;
                }

                self.annot.limpar();
                self.continuing = continuar;
                self.recorded_steps = 0;
                self.recorder = Some(recorder);

                if self.minimize_while_recording {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            }
            Err(err) => self.toasts.error(format!("não foi possível gravar: {err}")),
        }
    }

    /// Alterna entre pausado e gravando.
    pub fn toggle_pause(&mut self) {
        if let Some(recorder) = &self.recorder {
            recorder.toggle_pause();
        }
    }

    pub fn is_paused(&self) -> bool {
        self.recorder.as_ref().is_some_and(|r| r.is_paused())
    }

    pub fn stop_recording(&mut self, ctx: &egui::Context) {
        let Some(mut recorder) = self.recorder.take() else {
            return;
        };
        recorder.stop();
        // Drena o que a thread ainda tinha na fila antes de encerrar.
        let messages = recorder.messages.clone();
        self.drain_recorder_messages(&messages);

        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        let continuando = self.continuing;
        self.continuing = false;

        if let Some(project) = &mut self.project {
            project.recording.reindex();
            // Continuar deixa o foco no primeiro passo novo, que é onde a
            // atenção está; uma gravação nova começa do início.
            let inicio = if continuando {
                project
                    .recording
                    .steps
                    .len()
                    .saturating_sub(self.recorded_steps)
            } else {
                0
            };
            self.focused = project.recording.steps.get(inicio).map(|s| s.id);
        }
        self.selection = self.focused.into_iter().collect();
        self.screen = Screen::Editor;

        let total = self.recorded_steps;
        match (total, continuando) {
            (0, _) => self
                .toasts
                .info("nenhum passo foi capturado — nada aconteceu durante a gravação?"),
            (n, true) => self
                .toasts
                .info(format!("{n} passo(s) acrescentado(s) ao fim da gravação.")),
            (n, false) => self
                .toasts
                .info(format!("{n} passo(s) capturado(s). Revise e salve.")),
        }
    }

    /// Move os passos prontos da thread de captura para o projeto.
    fn drain_recorder_messages(
        &mut self,
        messages: &crossbeam_channel::Receiver<RecorderMessage>,
    ) {
        let Some(project) = &mut self.project else {
            return;
        };

        let mut novos = 0usize;
        let mut erros: Vec<String> = Vec::new();
        let mut avisos: Vec<String> = Vec::new();

        for message in messages.try_iter() {
            match message {
                RecorderMessage::Step(captured) => {
                    let captured = *captured;
                    if let (Some(image), Some(png)) = (&captured.step.image, captured.png) {
                        let path = image.path.clone();
                        let thumb_path = image.thumb_path.clone();
                        project.put_blob(path, png);
                        if let (Some(thumb_path), Some(thumb)) = (thumb_path, captured.thumb) {
                            project.put_blob(thumb_path, thumb);
                        }
                    }
                    project.recording.steps.push(captured.step);
                    project.recording.reindex();
                    novos += 1;
                }
                RecorderMessage::UpgradeLast { kind, caption } => {
                    if let Some(last) = project.recording.steps.last_mut() {
                        last.kind = kind;
                        if !last.caption_edited {
                            last.caption = caption;
                        }
                    }
                }
                // O pedido de parada é lido do sinalizador em `poll_recorder`,
                // que é onde dá para mexer na janela.
                RecorderMessage::StopRequested => {}
                // A tela lê o estado direto do gravador a cada quadro; a
                // mensagem existe para o aviso aparecer na hora.
                RecorderMessage::Paused(pausado) => {
                    avisos.push(if pausado {
                        format!("gravação pausada — {PAUSE_HOTKEY} para retomar")
                    } else {
                        "gravação retomada".to_string()
                    });
                }
                RecorderMessage::Error(err) => erros.push(err),
                RecorderMessage::Stopped => {}
            }
        }

        self.recorded_steps += novos;
        for aviso in avisos {
            self.toasts.info(aviso);
        }
        for err in erros {
            self.toasts.error(err);
        }
    }

    /// Chamado a cada quadro enquanto grava.
    fn poll_recorder(&mut self, ctx: &egui::Context) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        let stop_requested = recorder.stop_requested();
        // Clonar o `Receiver` é barato e solta o empréstimo de `self`.
        let messages = recorder.messages.clone();
        self.drain_recorder_messages(&messages);

        if stop_requested {
            self.stop_recording(ctx);
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    // ----- arquivo --------------------------------------------------------

    pub fn open_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Gravação StepEasy", &["stepeasy"])
            .pick_file()
        else {
            return;
        };
        self.open_path(&path);
    }

    /// Abre um `.stepeasy` de um caminho conhecido (diálogo ou linha de comando).
    pub fn open_path(&mut self, path: &std::path::Path) {
        match Project::open(path) {
            Ok(project) => {
                self.textures.clear();
                self.history.clear();
                self.selection.clear();
                self.annot.limpar();
                self.focused = project.recording.steps.first().map(|s| s.id);
                self.project = Some(project);
                self.screen = Screen::Editor;
                self.toasts.info(format!(
                    "aberto: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
            Err(err) => self.toasts.error(format!("não foi possível abrir: {err}")),
        }
    }

    pub fn save(&mut self, force_dialog: bool) {
        let Some(project) = &mut self.project else {
            return;
        };

        let path = if force_dialog || project.path().is_none() {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Gravação StepEasy", &["stepeasy"])
                .set_file_name(project.suggested_filename())
                .save_file()
            else {
                return;
            };
            Some(path)
        } else {
            None
        };

        let result = match path {
            Some(path) => project.save_as(path),
            None => project.save(),
        };

        match result {
            Ok(()) => {
                let nome = project
                    .path()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.toasts.info(format!("salvo em {nome}"));
            }
            Err(err) => self.toasts.error(format!("não foi possível salvar: {err}")),
        }
    }

    pub fn export(&mut self, format: Format) {
        let Some(project) = &mut self.project else {
            return;
        };

        let Some(path) = rfd::FileDialog::new()
            .add_filter(format.label(), &[format.extension()])
            .set_file_name(format!(
                "{}.{}",
                project.recording.slug(),
                format.extension()
            ))
            .save_file()
        else {
            return;
        };

        let recording = project.recording.clone();
        let result = match format {
            Format::Markdown => export::markdown::export(&recording, project, &path),
            Format::Html => export::html::export(&recording, project, &path),
        };

        match result {
            Ok(()) => self
                .toasts
                .info(format!("exportado para {}", path.display())),
            Err(err) => self.toasts.error(format!("falha ao exportar: {err}")),
        }
    }

    // ----- edição ---------------------------------------------------------

    pub fn undo(&mut self) {
        let Some(project) = &mut self.project else {
            return;
        };
        match self.history.undo(&mut project.recording) {
            Some(label) => {
                project.touch();
                self.sanitize_selection();
                self.toasts.info(format!("desfeito: {label}"));
            }
            None => self.toasts.info("nada para desfazer"),
        }
    }

    pub fn redo(&mut self) {
        let Some(project) = &mut self.project else {
            return;
        };
        match self.history.redo(&mut project.recording) {
            Some(label) => {
                project.touch();
                self.sanitize_selection();
                self.toasts.info(format!("refeito: {label}"));
            }
            None => self.toasts.info("nada para refazer"),
        }
    }

    /// Remove da seleção ids que não existem mais (depois de undo/exclusão).
    pub fn sanitize_selection(&mut self) {
        let Some(project) = &self.project else {
            self.selection.clear();
            self.focused = None;
            return;
        };
        let recording = &project.recording;
        self.selection.retain(|id| recording.position_of(*id).is_some());
        if self
            .focused
            .map(|id| recording.position_of(id).is_none())
            .unwrap_or(true)
        {
            self.focused = self
                .selection
                .first()
                .copied()
                .or_else(|| recording.steps.first().map(|s| s.id));
        }
    }

    /// Executa uma edição registrando no histórico e marcando o projeto sujo.
    pub fn edit<R>(
        &mut self,
        label: &str,
        f: impl FnOnce(&mut Recording) -> R,
    ) -> Option<R> {
        let project = self.project.as_mut()?;
        let out = self.history.edit(&mut project.recording, label, f);
        project.touch();
        Some(out)
    }

    /// Reaplica as legendas automáticas — usado depois de mudar a ordem.
    pub fn refresh_captions(&mut self) {
        if let Some(project) = &mut self.project {
            for step in &mut project.recording.steps {
                caption::refresh(step);
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.is_recording() {
            return;
        }
        let (save, open, undo, redo) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&KEY_SAVE),
                i.consume_shortcut(&KEY_OPEN),
                i.consume_shortcut(&KEY_UNDO),
                i.consume_shortcut(&KEY_REDO),
            )
        });
        if save {
            self.save(false);
        }
        if open {
            self.open_dialog();
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_recorder(&ctx);
        self.shortcuts(&ctx);

        screens::chrome::top_bar(self, ui);
        screens::chrome::status_bar(self, ui);

        match self.screen {
            Screen::Recorder => screens::recorder::show(self, ui),
            Screen::Editor => screens::editor::show(self, ui),
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("dark", if self.dark { "1" } else { "0" }.into());
        if let Ok(scope) = serde_json::to_string(&self.scope) {
            storage.set_string("scope", scope);
        }
    }
}

fn nome_padrao() -> String {
    format!("Gravação de {}", chrono::Local::now().format("%d/%m/%Y %H:%M"))
}
