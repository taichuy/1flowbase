// Diagnostic-only exclusive thread CPU accounting. Never merged into product.
#[derive(Default)]
pub(super) struct Accounting {
    stack: Vec<(u64, usize)>,
    last: u64,
}
impl Accounting {
    pub(super) fn current_stage(&self) -> Option<usize> {
        self.stack.last().map(|(_, stage)| *stage)
    }
    fn charge(&mut self, now: u64) -> Option<(usize, u64)> {
        let result = self
            .stack
            .last()
            .map(|(_, stage)| (*stage, now.saturating_sub(self.last)));
        self.last = now;
        result
    }
    pub(super) fn enter(&mut self, id: u64, stage: usize, now: u64) -> Option<(usize, u64)> {
        let cost = self.charge(now);
        self.stack.push((id, stage));
        cost
    }
    pub(super) fn exit(&mut self, id: u64, now: u64) -> (Option<(usize, u64)>, bool) {
        let cost = self.charge(now);
        let valid = self.stack.last().is_some_and(|(top, _)| *top == id);
        if valid {
            self.stack.pop();
        } else {
            self.stack.clear();
        }
        (cost, valid)
    }
}
#[cfg(test)]
#[path = "_tests/accounting.rs"]
mod tests;
