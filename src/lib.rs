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
pub struct Output {
    pub val: usize,
}

pub struct Analysis {}

#[derive(Debug)]
pub struct Task(pub usize);

impl From<Task> for usize {
    fn from(task: Task) -> usize {
        task.0
    }
}

impl Iterator for Work {
    type Item = Task;

    fn next(&mut self) -> Option<Task> {
        let n = self.cur;
        self.cur += 1;

        match n {
            n if n <= self.amount => Some(Task(n)),
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
