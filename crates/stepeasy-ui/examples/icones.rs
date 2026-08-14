//! Vitrine dos ícones desenhados, para conferir a olho como cada um fica.
//!
//! A fila de cima usa exatamente o botão que a interface usa; a de baixo é uma
//! ampliação, para inspecionar o traço.
//!
//! `cargo run -p stepeasy-ui --example icones`

use stepeasy_ui::{icons, ICONES};

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Ícones do StepEasy",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([520.0, 300.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            stepeasy_ui::aplicar_tema(&cc.egui_ctx, true);
            Ok(Box::new(Vitrine))
        }),
    )
}

struct Vitrine;

impl eframe::App for Vitrine {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Tamanho real (o botão que a interface usa)");
            ui.horizontal(|ui| {
                for icon in ICONES {
                    let _ = icons::button(ui, *icon);
                }
            });

            ui.add_space(16.0);
            ui.label("Ampliado");
            let cor = ui.style().visuals.text_color();
            ui.horizontal(|ui| {
                for icon in ICONES {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(56.0), egui::Sense::hover());
                    icons::draw(ui.painter(), *icon, rect.center(), 18.0, cor);
                }
            });
        });
    }
}
