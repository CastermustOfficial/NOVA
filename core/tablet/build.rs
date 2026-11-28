fn main() {
    // Il microfono su Android passa da Oboe, che e' C++ (D455). Chi lo
    // costruisce non collega la libreria del C++ in modo che arrivi fino a
    // qui: senza queste due righe il programma si costruisce, si installa, e
    // all'avvio non si carica («cannot locate symbol __cxa_pure_virtual»).
    // Il 28 novembre l'ho installato cosi' sul tablet, che e' rimasto quasi
    // due minuti senza NOVA. Statica, per non portare un'altra libreria
    // nell'APK; in fondo alla riga del collegamento, dopo chi la usa.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-arg=-lc++_static");
        println!("cargo:rustc-link-arg=-lc++abi");
    }
    tauri_build::build()
}
