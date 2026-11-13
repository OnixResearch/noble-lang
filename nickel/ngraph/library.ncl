let parameter_fn = fun parameter_name parameter_value => {
  name = parameter_name,
  value = parameter_value,
} in
let handle_fn = fun scope_id local_id => { scope = scope_id, id = local_id } in
let input_fn = fun node_id => { id = node_id, op = { kind = "input" } } in
let constant_fn = fun node_id value_text => {
  id = node_id,
  op = { kind = "const", value = value_text },
} in
let add_fn = fun node_id left_id right_id => {
  id = node_id,
  op = { kind = "add", left = left_id, right = right_id },
} in
let sub_fn = fun node_id left_id right_id => {
  id = node_id,
  op = { kind = "sub", left = left_id, right = right_id },
} in
let mul_fn = fun node_id left_id right_id => {
  id = node_id,
  op = { kind = "mul", left = left_id, right = right_id },
} in
let template_fn = fun template_name parameter_data input_data node_data output_data => {
  name = template_name,
  parameters = parameter_data,
  inputs = input_data,
  nodes = node_data,
  outputs = output_data,
} in
let instantiate_fn = fun scope_id t => {
  scope = scope_id,
  template = t.name,
  inputs = t.inputs,
  outputs = t.outputs,
  nodes = t.nodes,
} in
let wire_fn = fun source_handle target_handle => {
  from = source_handle,
  to = target_handle,
} in
let contract = import "contract.ncl" in
let bundle_fn = fun parameter_data instance_data wire_data input_data output_data => {
  schema = "noble-ngraph-nickel/v1",
  library_version = "noble-ngraph-nickel/1",
  contract_version = "noble-ngraph-nickel-contracts/1",
  profile = "ngraph-i64/0",
  bundle_sha256 = "",
  parameters = parameter_data,
  instances = instance_data,
  wires = wire_data,
  inputs = input_data,
  outputs = output_data,
} | contract in
{
  parameter = parameter_fn,
  handle = handle_fn,
  input = input_fn,
  constant = constant_fn,
  add = add_fn,
  sub = sub_fn,
  mul = mul_fn,
  template = template_fn,
  instantiate = instantiate_fn,
  wire = wire_fn,
  bundle = bundle_fn,
}
