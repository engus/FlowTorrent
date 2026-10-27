fn main() {
    slint_build::compile("ui/probe.slint").expect("compile engine probe UI");
    slint_build::compile("ui/main.slint").expect("compile FlowTorrent UI");
}
