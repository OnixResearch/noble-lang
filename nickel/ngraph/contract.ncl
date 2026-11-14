let handle = {
  scope | String,
  id | String,
} in
let node = {
  id | String,
  op | { kind | String, .. },
} in
let instance = {
  scope | String,
  template | String,
  inputs | Array String,
  outputs | Array String,
  nodes | Array node,
} in
{
  schema | String,
  library_version | String,
  contract_version | String,
  profile | String,
  bundle_sha256 | String,
  parameters | Array { name | String, value | String },
  instances | Array instance,
  wires | Array { from | handle, to | handle },
  inputs | Array handle,
  outputs | Array handle,
}
