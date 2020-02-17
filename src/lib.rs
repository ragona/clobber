#[derive(Debug)]
pub struct Work {
    cur: usize,
    amount: usize,
}

impl Work {
    pub fn new(amount: usize) -> Self {
        Self { cur: 0, amount }
    }
}

#[derive(Debug)]
pub struct Output {}
pub struct Analysis {}

impl Iterator for Work {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        let n = self.cur;
        self.cur += 1;

        match n {
            n if n <= self.amount => Some(n),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    pub fn foo() {
        let work = Work::new(5);

        for i in work {
            dbg!(i);
        }
    }
}
