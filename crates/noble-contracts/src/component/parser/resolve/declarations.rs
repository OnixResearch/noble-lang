#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; bounded interface, function and world scans reject duplicate names and resource/function collisions as WIT diagnostics; assertions cannot validate hostile declarations."
)]
pub(super) fn unique(package: &super::super::Package) -> Result<(), crate::component::Error> {
    let mut at = 0usize;
    let mut failure = None;
    while at < package.interfaces.len() {
        if let Err(problem) = interface_names(package, at) {
            failure = Some(problem);
            break;
        }
        at = at.saturating_add(1);
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    at = 0;
    while at < package.worlds.len() {
        if let Err(problem) = world_name(package, at) {
            failure = Some(problem);
            break;
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the caller supplies an admitted declaration index, and the bounded scan compares only earlier owned interface names before validating that interface's functions."
)]
fn interface_names(
    package: &super::super::Package,
    at: usize,
) -> Result<(), crate::component::Error> {
    let interface = &package.interfaces[at];
    let mut previous = 0usize;
    let mut is_duplicate = false;
    while previous < at {
        if same_interface_name(package, previous, at) {
            is_duplicate = true;
            break;
        }
        previous = previous.saturating_add(1);
    }
    if is_duplicate {
        return Err(crate::component::invalid(
            "duplicate WIT interface or world",
        ));
    }
    let mut index = 0usize;
    let mut failure = None;
    while index < interface.functions.len() {
        if let Err(problem) = function_name(interface, index) {
            failure = Some(problem);
            break;
        }
        index = index.saturating_add(1);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the current and earlier function indices are bounded by the caller's owned interface, and duplicate or colliding names are ordinary WIT input errors."
)]
fn function_name(
    interface: &super::super::Interface,
    at: usize,
) -> Result<(), crate::component::Error> {
    let function = &interface.functions[at];
    let mut previous = 0usize;
    let mut is_duplicate = false;
    while previous < at {
        if same_function(&interface.functions[previous], function) {
            is_duplicate = true;
            break;
        }
        previous = previous.saturating_add(1);
    }
    if is_duplicate {
        return Err(crate::component::invalid("duplicate WIT function"));
    }
    if function.method.is_none() && interface.resources.contains(&function.name) {
        return Err(crate::component::invalid(
            "WIT function and resource share a name",
        ));
    }
    Ok(())
}

#[expect(
    tigerstyle::ambiguous_params,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; these two admitted indices are symmetric equality operands, so swapping them does not change the result; String content equality is non-const."
)]
/// Whether two interfaces of `package` share a name.
fn same_interface_name(package: &super::super::Package, left: usize, right: usize) -> bool {
    package.interfaces[left].name == package.interfaces[right].name
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; Option<String> and String content equality are non-const trait calls."
)]
/// Whether two functions share a method scope and a name.
fn same_function(old: &super::super::Function, function: &super::super::Function) -> bool {
    old.method == function.method && old.name == function.name
}

#[expect(
    tigerstyle::ambiguous_params,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the interface and world indices address different tables and are named at the single call site; String content equality is non-const."
)]
/// Whether interface `interface` of `package` is named like world `world`.
fn interface_names_world(package: &super::super::Package, interface: usize, world: usize) -> bool {
    package.interfaces[interface].name == package.worlds[world].name
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; collisions and duplicates are ordinary malformed WIT and return typed errors; assertions would turn required input rejection into panics."
)]
fn world_name(package: &super::super::Package, at: usize) -> Result<(), crate::component::Error> {
    if interface_world_collision(package, at) {
        return Err(crate::component::invalid(
            "duplicate WIT interface or world",
        ));
    }
    let mut previous = 0usize;
    let mut is_duplicate = false;
    while previous < at {
        if same_world_name(package, previous, at) {
            is_duplicate = true;
            break;
        }
        previous = previous.saturating_add(1);
    }
    if is_duplicate {
        return Err(crate::component::invalid(
            "duplicate WIT interface or world",
        ));
    }
    world_names(&package.worlds[at])
}

fn interface_world_collision(package: &super::super::Package, at: usize) -> bool {
    let mut is_found = false;
    let mut index = 0;
    while !is_found && index < package.interfaces.len() {
        is_found = interface_names_world(package, index, at);
        index += 1;
    }
    is_found
}

#[expect(
    tigerstyle::ambiguous_params,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; these two admitted indices are symmetric equality operands, so swapping them does not change the result; String content equality is non-const."
)]
fn same_world_name(package: &super::super::Package, left: usize, right: usize) -> bool {
    package.worlds[left].name == package.worlds[right].name
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each bounded item is checked against earlier names and duplicate input returns a typed error before world publication."
)]
fn world_names(world: &super::super::RawWorld) -> Result<(), crate::component::Error> {
    let mut at = 0usize;
    let mut failure = None;
    while at < world.items.len() {
        let mut previous = 0usize;
        let mut is_duplicate = false;
        while previous < at {
            if same_item_name(&world.items[previous], &world.items[at]) {
                is_duplicate = true;
                break;
            }
            previous = previous.saturating_add(1);
        }
        if is_duplicate {
            failure = Some(crate::component::invalid("duplicate WIT world item"));
            break;
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; duplicate-key comparison invokes non-const tuple and string content equality over borrowed parsed declarations."
)]
fn same_item_name(left: &super::super::Item, right: &super::super::Item) -> bool {
    item_key(left) == item_key(right)
}

const fn item_key(item: &super::super::Item) -> (bool, &str) {
    match item {
        super::super::Item::Interface(is_imported, name) => (*is_imported, name.as_str()),
        super::super::Item::Function(is_imported, function) => {
            (*is_imported, function.name.as_str())
        }
        super::super::Item::Use(usage) => (true, usage.name.as_str()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; bounded resource registration checks capacity and integer conversion before publishing the complete table; failures return diagnostics without partial publication."
)]
pub(super) fn resources(
    package: &super::super::Package,
) -> Result<alloc::vec::Vec<crate::component::Resource>, crate::component::Error> {
    let mut result = alloc::vec::Vec::new();
    let mut at = 0usize;
    let mut failure = None;
    while let Some(interface) = package.interfaces.get(at) {
        let mut index = 0usize;
        while let Some(name) = interface.resources.get(index) {
            if result.len() >= crate::component::MAX_RESOURCES {
                failure = Some(crate::component::exhausted());
                break;
            }
            let kind = match u32::try_from(result.len().saturating_add(1)) {
                Ok(kind) => noble_kernel::types::ResourceKind(kind),
                Err(_) => {
                    failure = Some(crate::component::exhausted());
                    break;
                }
            };
            result.push(crate::component::Resource {
                identity: super::joined(&super::qualified(package, &interface.name), "#", name),
                interface: interface.name.clone(),
                name: name.clone(),
                kind,
            });
            index = index.saturating_add(1);
        }
        if failure.is_some() {
            break;
        }
        at = at.saturating_add(1);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(result),
    }
}
