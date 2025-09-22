pub trait HandleFrame {
    fn handle_frame(&mut self, parts: &[&str], value: Option<f64>);
}
