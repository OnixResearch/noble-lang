//! Frozen complete and seeded cohorts.
use super::{compare, initial, Action, Limits, Rights};

#[path = "generated/boundary.rs"]
mod boundary;

const ENUM_LIMITS: Limits = Limits {
    facets: 3,
    assertions: 2,
    interests: 1,
    events: 8,
};
const EXPANDED: Limits = Limits {
    facets: 8,
    assertions: 8,
    interests: 8,
    events: 32,
};
const SEEDS: [u64; 3] = [0x00000000130a1301, 0x00000000130b1302, 0x00000000130c1303];

fn enum_action(digit: u32) -> Action {
    match digit {
        0 => Action::Publish {
            who: 1,
            name: "clock",
            ready: true,
        },
        1 => Action::Publish {
            who: 2,
            name: "clock",
            ready: true,
        },
        2 => Action::Observe {
            who: 3,
            name: "clock",
            ready: true,
        },
        3 => Action::Retract {
            who: 1,
            name: "clock",
        },
        4 => Action::Retract {
            who: 2,
            name: "clock",
        },
        _ => Action::Drain,
    }
}

#[test]
fn complete_small_alphabet() {
    let mut total = 0;
    let mut word = String::with_capacity(4);
    for length in 0..=4_u32 {
        let count = 6_u32.pow(length);
        for ordinal in 0..count {
            let mut actions = initial().to_vec();
            let mut digits = ordinal;
            word.clear();
            for _ in 0..length {
                let digit = digits % 6;
                digits /= 6;
                actions.push(enum_action(digit));
                word.push(char::from(
                    b'0'.saturating_add(u8::try_from(digit).expect("base-six digit")),
                ));
            }
            compare(&format!("enum/{length}/{word}"), ENUM_LIMITS, &actions);
            total += 1;
        }
    }
    assert_eq!(total, 1_555);
}

struct Generator(u64);

impl Generator {
    fn draw(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        u32::try_from(self.0 >> 32).expect("high half is u32")
    }
}

fn generated_action(random: &mut Generator) -> Action {
    let pick = random.draw() % 18;
    let name = if random.draw().is_multiple_of(3) {
        "alarm"
    } else {
        "clock"
    };
    let is_ready = random.draw().is_multiple_of(2);
    match pick {
        0..=5 => Action::Publish {
            who: if pick.is_multiple_of(2) { 1 } else { 2 },
            name,
            ready: is_ready,
        },
        6..=8 => Action::Observe {
            who: if pick == 8 { 4 } else { 3 },
            name: "clock",
            ready: is_ready,
        },
        9..=10 => Action::Retract {
            who: if pick == 9 { 1 } else { 2 },
            name,
        },
        11 => Action::Retire { who: 1 },
        12 => Action::Retire { who: 2 },
        13 => Action::Retire { who: 3 },
        14 => Action::Drain,
        15 => Action::Open {
            parent: None,
            rights: Rights::Publish,
        },
        16 => Action::Open {
            parent: Some(1),
            rights: Rights::Observe,
        },
        _ => Action::Observe {
            who: 4,
            name: "clock",
            ready: is_ready,
        },
    }
}

#[test]
fn seeded_expanded_histories() {
    for seed in SEEDS {
        let mut random = Generator(seed);
        for ordinal in 0..96 {
            let mut actions = vec![
                Action::Open {
                    parent: None,
                    rights: Rights::Publish,
                },
                Action::Open {
                    parent: Some(1),
                    rights: Rights::Publish,
                },
                Action::Open {
                    parent: None,
                    rights: Rights::Observe,
                },
                Action::Open {
                    parent: None,
                    rights: Rights::Observe,
                },
            ];
            for _ in 0..16 {
                actions.push(generated_action(&mut random));
            }
            compare(
                &format!("seed/{seed:016x}/{ordinal:03}"),
                EXPANDED,
                &actions,
            );
        }
    }
}
