use wgsl_workbench::run;

fn main() {
    pollster::block_on(run());
}
