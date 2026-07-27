use std::process::Child;

pub struct SubprocessGuard {
    child: Child,
}

impl SubprocessGuard {
    pub fn new(child: Child) -> Self {
        Self { child }
    }
}

impl Drop for SubprocessGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
