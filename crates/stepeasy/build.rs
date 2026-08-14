//! Embute o ícone e os metadados no executável do Windows.
//!
//! É o que faz o Explorer, a barra de tarefas e o Alt+Tab mostrarem o logo em
//! vez do ícone genérico. Nos outros sistemas não há nada a fazer aqui.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/stepeasy.ico");

    #[cfg(windows)]
    {
        let mut recurso = winresource::WindowsResource::new();
        recurso.set_icon("../../assets/icons/stepeasy.ico");
        recurso.set("FileDescription", "StepEasy — gravador de passo a passo");
        recurso.set("ProductName", "StepEasy");
        recurso.set("LegalCopyright", "MIT");
        if let Err(err) = recurso.compile() {
            // Sem o ícone o aplicativo funciona igual; não vale quebrar a
            // compilação de quem não tem as ferramentas de recurso do Windows.
            println!("cargo:warning=não foi possível embutir o ícone: {err}");
        }
    }
}
