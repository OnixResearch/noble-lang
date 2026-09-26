//! Fixed-capacity strict JSON ingress for the one selected M8 descriptor.
//! This intentionally parses no general JSON values: every decoded key is
//! matched to a typed field immediately, with no map, heap string or Vec.

use super::*;

mod scan;

pub(super) struct Parsed {
    pub rounds: [Option<Round>; 2],
    pub len: usize,
}

struct Decoder<'a> {
    source: &'a str,
    input: &'a [u8],
    at: usize,
    // Escaped strings are decoded into one reused bounded scratch region.
    text: Option<[u8; MAX_DESCRIPTOR_BYTES]>,
}

pub(super) fn parse(input: &[u8]) -> Result<Parsed, Error> {
    let source = attempt!(scan::preflight(input));
    Decoder {
        source,
        input,
        at: 0,
        text: None,
    }
    .root()
}

impl Decoder<'_> {
    fn round(&mut self) -> Result<Round, Error> {
        attempt!(self.require(b'{'));
        let (mut name, mut ready, mut exit) = (None, None, None);
        loop {
            if self.take(b'}') {
                break;
            }
            let key = match attempt!(self.string()) {
                "name" => 0,
                "ready" => 1,
                "exit" => 2,
                _ => return Err(Error::Descriptor),
            };
            attempt!(self.require(b':'));
            match key {
                0 if name.is_none() => {
                    let value = attempt!(self.string());
                    name = Some(attempt!(Name::checked(value).ok_or(Error::Descriptor)))
                }
                1 if ready.is_none() => ready = Some(attempt!(self.boolean())),
                2 if exit.is_none() => {
                    exit = Some(match attempt!(self.string()) {
                        "withdraw" => Exit::Withdraw,
                        "trap" => Exit::Trap,
                        _ => return Err(Error::Descriptor),
                    });
                }
                _ => return Err(Error::Descriptor), // decoded duplicate key
            }
            if self.take(b'}') {
                break;
            }
            attempt!(self.require(b','));
            if self.take(b'}') {
                return Err(Error::Descriptor);
            }
        }
        Ok(Round {
            name: attempt!(name.ok_or(Error::Descriptor)),
            ready: attempt!(ready.ok_or(Error::Descriptor)),
            exit: attempt!(exit.ok_or(Error::Descriptor)),
        })
    }

    fn rounds(&mut self) -> Result<Parsed, Error> {
        attempt!(self.require(b'['));
        let mut rounds = [None, None];
        let mut len = 0;
        if self.take(b']') {
            return Err(Error::Descriptor);
        }
        loop {
            if len == rounds.len() {
                return Err(Error::Descriptor);
            }
            rounds[len] = Some(attempt!(self.round()));
            len += 1;
            if self.take(b']') {
                break;
            }
            attempt!(self.require(b','));
            if self.take(b']') {
                return Err(Error::Descriptor);
            }
        }
        Ok(Parsed { rounds, len })
    }

    fn root(&mut self) -> Result<Parsed, Error> {
        attempt!(self.require(b'{'));
        let mut is_protocol = None;
        let mut rounds = None;
        loop {
            if self.take(b'}') {
                break;
            }
            let key = match attempt!(self.string()) {
                "protocol" => 0,
                "rounds" => 1,
                _ => return Err(Error::Descriptor),
            };
            attempt!(self.require(b':'));
            match key {
                0 if is_protocol.is_none() => {
                    is_protocol = Some(attempt!(self.string()) == PROTOCOL)
                }
                1 if rounds.is_none() => rounds = Some(attempt!(self.rounds())),
                _ => return Err(Error::Descriptor), // decoded duplicate key
            }
            if self.take(b'}') {
                break;
            }
            attempt!(self.require(b','));
            if self.take(b'}') {
                return Err(Error::Descriptor);
            }
        }
        let parsed = attempt!(rounds.ok_or(Error::Descriptor));
        let is_protocol = attempt!(is_protocol.ok_or(Error::Descriptor));
        self.whitespace();
        if self.at != self.input.len() {
            return Err(Error::ParserEnd);
        }
        if !is_protocol {
            return Err(Error::Protocol);
        }
        Ok(parsed)
    }
}
