use log::debug;

#[derive(Debug)]
enum ControllerType {
    Proportional,
    Integral,
    Derivative,
}

struct Controller {
    pub controller_type: ControllerType,
    pub gain: f32,
    pub error: f32,
}

impl Controller {
    pub fn new(controller_type: ControllerType, gain: f32) -> Self {
        Self {
            controller_type,
            gain,
            error: 0.0,
        }
    }

    pub fn update(&mut self, error: f32) {
        self.error = match self.controller_type {
            ControllerType::Proportional => error,
            ControllerType::Integral => (error + self.error) / 2.0,
            ControllerType::Derivative => error - self.error,
        };

        debug!("{:#?}, {}", self.controller_type, self.error);
    }

    pub fn output(&self) -> f32 {
        self.error * self.gain
    }
}

pub struct PidController {
    p: Controller,
    i: Controller,
    d: Controller,
}

impl PidController {
    /// Creates a new PidController with the provided `gain` tuple.
    /// Gain is used to balance the respective volume of each controller.
    pub fn new(gain: (f32, f32, f32)) -> Self {
        let (p_gain, i_gain, d_gain) = gain;
        Self {
            p: Controller::new(ControllerType::Proportional, p_gain),
            i: Controller::new(ControllerType::Integral, i_gain),
            d: Controller::new(ControllerType::Derivative, d_gain),
        }
    }

    pub fn update(&mut self, goal: f32, current: f32) {
        let error = goal - current;

        self.p.update(error);
        self.i.update(error);
        self.d.update(error);

        debug!("PidController, {}", self.output());
    }

    pub fn output(&self) -> f32 {
        self.p.output() + self.i.output() + self.d.output()
    }
}
