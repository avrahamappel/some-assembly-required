use std::cell::RefCell;
use std::io;
use std::rc::Rc;

type WireName = Rc<str>;
type WireRef = Rc<RefCell<Wire>>;
type Signal = u16;

#[derive(Debug)]
enum Connection {
    Name(WireName),
    Wire(WireRef),
}

impl Connection {
    fn get_signal(&self) -> Option<Signal> {
        match self {
            Self::Name(_) => None,
            Self::Wire(w) => w.borrow().signal,
        }
    }
}

impl From<&str> for Connection {
    fn from(name: &str) -> Self {
        Self::Name(name.into())
    }
}

#[derive(Debug)]
enum Input {
    Signal(Signal),
    Connection(Connection),
}

impl Input {
    fn get_signal(&self) -> Option<Signal> {
        match self {
            Self::Signal(s) => Some(*s),
            Self::Connection(c) => c.get_signal(),
        }
    }
}

impl From<&str> for Input {
    fn from(input: &str) -> Self {
        match input.parse::<Signal>() {
            Ok(s) => Self::Signal(s),
            Err(_) => Self::Connection(input.into()),
        }
    }
}

#[derive(Debug)]
enum Gate {
    Single(Input),
    And(Input, Input),
    Or(Input, Input),
    RShift(Input, u16),
    LShift(Input, u16),
    Not(Input),
}

impl Gate {
    fn parse(string: &str) -> Self {
        match string.split(' ').collect::<Vec<_>>()[..] {
            [a, "AND", b] => Self::And(a.into(), b.into()),
            [a, "OR", b] => Self::Or(a.into(), b.into()),
            [w, "RSHIFT", s] => Self::RShift(w.into(), s.parse::<u16>().unwrap()),
            [w, "LSHIFT", s] => Self::LShift(w.into(), s.parse::<u16>().unwrap()),
            ["NOT", w] => Self::Not(w.into()),
            [s] => Self::Single(s.into()),
            _ => unimplemented!(),
        }
    }

    fn connect(&mut self, wires: &[WireRef]) {
        fn find_wire(connection: &Connection, wires: &[WireRef]) -> Connection {
            match connection {
                Connection::Wire(w) => Connection::Wire(w.clone()),
                Connection::Name(name) => wires
                    .iter()
                    // try_borrow could fail if it's the current wire (that this gate belongs to)
                    .find(|w| w.try_borrow().map(|w| w.name == *name).unwrap_or(false))
                    // Connect the input wire as well
                    .map(|w| {
                        w.borrow_mut().connect(wires);
                        w
                    })
                    .map(Rc::clone)
                    .map(Connection::Wire)
                    .unwrap_or_else(|| Connection::Name(name.clone())),
            }
        }

        match self {
            Self::And(Input::Connection(a), Input::Connection(b))
            | Self::Or(Input::Connection(a), Input::Connection(b)) => {
                *a = find_wire(a, wires);
                *b = find_wire(b, wires);
            }
            Self::And(Input::Connection(a), Input::Signal(_))
            | Self::And(Input::Signal(_), Input::Connection(a))
            | Self::Or(Input::Connection(a), Input::Signal(_))
            | Self::Or(Input::Signal(_), Input::Connection(a))
            | Self::RShift(Input::Connection(a), _)
            | Self::LShift(Input::Connection(a), _)
            | Self::Not(Input::Connection(a))
            | Self::Single(Input::Connection(a)) => {
                *a = find_wire(a, wires);
            }
            _ => {}
        }
    }

    fn get_signal(&self) -> Option<Signal> {
        match self {
            Self::Single(s) => s.get_signal(),
            Self::And(a, b) => a.get_signal().and_then(|a| b.get_signal().map(|b| a & b)),
            Self::Or(a, b) => a.get_signal().and_then(|a| b.get_signal().map(|b| a | b)),
            Self::RShift(s, o) => s.get_signal().map(|s| s >> o),
            Self::LShift(s, o) => s.get_signal().map(|s| s << o),
            Self::Not(s) => s.get_signal().map(|s| !s),
        }
    }
}

#[derive(Debug)]
struct Wire {
    name: WireName,
    gate: Gate,
    signal: Option<Signal>,
}

impl Wire {
    fn parse(string: &str) -> Self {
        let (cmd, name) = string.split_once(" -> ").unwrap();
        let gate = Gate::parse(cmd);

        Self {
            name: name.into(),
            gate,
            signal: None,
        }
    }

    fn connect(&mut self, wires: &[WireRef]) {
        if self.signal.is_some() {
            return;
        }

        self.gate.connect(wires);

        self.signal = self.gate.get_signal();

        macro_rules! print_input {
            ($input:ident) => {
                match $input {
                    Input::Signal(s) => format!("signal {}", s),
                    Input::Connection(c) => match c {
                        Connection::Name(_) => format!("unknown"),
                        Connection::Wire(w) => format!("'{}'", w.borrow().name),
                    },
                }
            };
        }

        println!(
            "'{:>2}' is connected to {:>25}, and has {}",
            self.name,
            match &self.gate {
                Gate::Single(i) => print_input!(i),
                Gate::And(a, b) => format!("{} AND {}", print_input!(a), print_input!(b)),
                Gate::Or(a, b) => format!("{} OR {}", print_input!(a), print_input!(b)),
                Gate::RShift(i, s) => format!("{} right-shifted by {}", print_input!(i), s),
                Gate::LShift(i, s) => format!("{} left-shifted by {}", print_input!(i), s),
                Gate::Not(i) => format!("NOT {}", print_input!(i)),
            },
            match self.signal {
                Some(s) => format!("signal {}", s),
                None => "no signal".to_string(),
            }
        );
    }
}

struct Circuit {
    wires: Vec<WireRef>,
}

impl Circuit {
    fn assemble<I>(w_iter: I) -> Self
    where
        I: Iterator<Item = Wire>,
    {
        let wires = w_iter.map(|w| Rc::new(RefCell::new(w))).collect::<Vec<_>>();

        for wire in &wires {
            wire.borrow_mut().connect(&wires);
        }

        Self { wires }
    }

    fn get(&self, name: &str) -> Option<Signal> {
        self.wires
            .iter()
            .find(|w| w.borrow().name == name.into())
            .and_then(|w| w.borrow().signal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_circuit() {
        let input = "123 -> x
456 -> y
x AND y -> d
x OR y -> e
x LSHIFT 2 -> f
y RSHIFT 2 -> g
NOT x -> h
NOT y -> i";

        let output = "d: 72
e: 507
f: 492
g: 114
h: 65412
i: 65079
x: 123
y: 456";

        let circuit = Circuit::assemble(input.lines().map(Wire::parse));

        let mut wires = circuit
            .wires
            .iter()
            .map(|w| {
                format!(
                    "{}: {}",
                    w.borrow().name,
                    match w.borrow().signal {
                        None => String::new(),
                        Some(s) => s.to_string(),
                    }
                )
            })
            .collect::<Vec<_>>();
        wires.sort();

        assert_eq!(output.split('\n').collect::<Vec<_>>(), wires);
    }
}

fn main() {
    let w_iter = io::stdin()
        .lines()
        .map(Result::unwrap)
        .map(|s| Wire::parse(&s));
    let circuit = Circuit::assemble(w_iter);

    println!("CIRCUIT 'a': {}", circuit.get("a").unwrap());
}
