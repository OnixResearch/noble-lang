use noble_kernel::resources::{Context, Error, Handle, Limits, Requirement, Rights, State, Table, TableId};
use noble_kernel::types::ResourceKind;

fn main() -> Result<(), String> {
    let representation = std::env::args().nth(1).ok_or("missing handle representation")?
        .parse::<usize>().map_err(|error| format!("invalid handle representation: {error}"))?;
    if std::env::args().count() != 2 {
        return Err("usage: handle-check REPRESENTATION".into());
    }
    let required = Requirement { context: Context(7), kind: ResourceKind(1), rights: Rights(1) };
    let mut table = Table::new(TableId(11), Limits {
        slots: representation.checked_add(2).ok_or("representation overflows table capacity")?,
        pins: 1, owners_per_context: 1, generations: 2, scopes: 2,
    }).map_err(|error| format!("table setup: {error:?}"))?;
    let forged = Handle { table: TableId(11), slot: representation,
        generation: 1, context: required.context, kind: required.kind, rights: required.rights };
    if table.validate(forged, required) != Err(Error::InvalidHandle) {
        return Err("absent forged representation did not reject as invalid handle".into());
    }
    let empty = table.observation();
    if empty.live != 0 || empty.busy != 0 || empty.native_pins != 0 {
        return Err(format!("rejected handle changed ownership: {empty:?}"));
    }
    // A legitimate owner can only be minted by trusted registration. The
    // unrelated forged slot must remain absent even after a real registration.
    let owner = table.register(required).map_err(|error| format!("register: {error:?}"))?;
    if owner.handle().slot == representation {
        return Err("positive control unexpectedly populated the forged slot".into());
    }
    if table.validate(owner.handle(), required).map_err(|error| format!("valid owner: {error:?}"))?.state != State::Live {
        return Err("legitimate registered owner was not live".into());
    }
    if table.validate(forged, required) != Err(Error::InvalidHandle) {
        return Err("forged slot became valid after legitimate registration".into());
    }
    if table.observation().native_pins != 0 {
        return Err("read-only validation acquired native pin".into());
    }
    table.release(owner, required).map_err(|error| format!("release: {:?}", error.error))?;
    println!("{{\"id\":\"S-CASE-03\",\"stage\":\"adapter\",\"outcome\":\"invalid-handle\",\"protected_operations\":0,\"positive_registered_owner\":true,\"absent_after_registration\":true}}");
    Ok(())
}
