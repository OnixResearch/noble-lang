impl super::Cursor<'_> {
    pub(super) fn function(
        &mut self,
        name: alloc::string::String,
        method: Option<alloc::string::String>,
    ) -> Result<super::Function, crate::component::Error> {
        attempt!(self.take(":"));
        if self.peek("async") {
            return Err(crate::component::unsupported("async and suspension, including async with borrow, are outside the synchronous boundary"));
        }
        attempt!(self.take("func"));
        attempt!(self.take("("));
        let mut parameters = alloc::vec::Vec::new();
        let mut names = alloc::vec::Vec::with_capacity(crate::component::MAX_PARAMETERS);
        if let Some(resource) = &method {
            parameters.push(super::RawType::Borrow(resource.clone()));
        }
        let mut failure = None;
        while parameters.len() < crate::component::MAX_PARAMETERS && !self.peek(")") {
            match self.parameter(&names) {
                Ok((name, ty)) => {
                    names.push(name);
                    parameters.push(ty);
                }
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        if !self.peek(")") {
            return Err(crate::component::exhausted());
        }
        attempt!(self.take(")"));
        let mut results = alloc::vec::Vec::new();
        if self.peek("->") {
            attempt!(self.take("->"));
            results.push(attempt!(self.ty()));
        }
        attempt!(self.take(";"));
        Ok(super::Function {
            name,
            method,
            parameters,
            results,
        })
    }

    fn parameter(
        &mut self,
        names: &[alloc::string::String],
    ) -> Result<(alloc::string::String, super::RawType), crate::component::Error> {
        let name = attempt!(self.name());
        if names.contains(&name) {
            return Err(crate::component::invalid("duplicate WIT parameter"));
        }
        attempt!(self.take(":"));
        let ty = attempt!(self.ty());
        if !self.peek(")") {
            attempt!(self.take(","));
        }
        Ok((name, ty))
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; type parsing consumes metered tokens, allocates owned resource names and returns owned diagnostics for unsupported adapters."
    )]
    fn ty(&mut self) -> Result<super::RawType, crate::component::Error> {
        let name = attempt!(self.next());
        match name {
            "bool" => Ok(super::RawType::Bool),
            "s64" => Ok(super::RawType::S64),
            "string" => Ok(super::RawType::String),
            "list" => {
                attempt!(self.take("<"));
                attempt!(self.take("u8"));
                attempt!(self.take(">"));
                Ok(super::RawType::Bytes)
            }
            "result" => {
                attempt!(self.take("<"));
                attempt!(self.take("s64"));
                attempt!(self.take(","));
                attempt!(self.take("string"));
                attempt!(self.take(">"));
                Ok(super::RawType::ResultS64String)
            }
            "own" | "borrow" => {
                attempt!(self.take("<"));
                let resource = attempt!(self.name());
                attempt!(self.take(">"));
                if name == "own" {
                    Ok(super::RawType::Own(resource))
                } else {
                    Ok(super::RawType::Borrow(resource))
                }
            }
            "u8" | "s8" | "u16" | "s16" | "u32" | "s32" | "u64" | "f32" | "f64" | "char"
            | "tuple" | "option" | "future" | "stream" => Err(crate::component::unsupported(
                "WIT type has no exact supported synchronous adapter",
            )),
            _ => {
                self.at = self.at.saturating_sub(1);
                Ok(super::RawType::Own(attempt!(self.name())))
            }
        }
    }
}
