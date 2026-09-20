# File adapter for the pure comparator. Used only by the build/test driver.
{
  directory,
  case,
  policy,
  selection,
}:
let
  read = name: builtins.fromJSON (builtins.readFile (directory + "/" + name));
  has = name: builtins.pathExists (directory + "/" + name);
  precollection = case.observation == "precollection";
  denyStatus = if has "deny/status.json" then read "deny/status.json" else { };
  denyRaw = if has "deny/results.jsonl" then builtins.readFile (directory + "/deny/results.jsonl") else "";
  lines = builtins.filter (line: builtins.isString line && line != "") (builtins.split "\n" denyRaw);
  compilerOnly = case.observation == "compiler_only";
  observationDirectory =
    if
      case.observation == "clean"
      || case.observation == "role_edge"
      || (case.observation == "policy" && case.deny_lint == "")
    then
      "deny"
    else
      "observation";
  observed = {
    deny_exit = read "deny-exit.json";
    deny_status = denyStatus;
    deny_findings =
      if precollection then
        [ ]
      else if denyStatus ? phases then
        (builtins.fromJSON denyRaw).lint_findings
      else
        map builtins.fromJSON lines;
  }
  // (
    if precollection then
      {
        precollection = {
          exit = read "deny-exit.json";
          diagnostic = builtins.readFile (directory + "/precollection-diagnostic.txt");
          status_published = has "deny/status.json";
        };
      }
    else if compilerOnly then
      { }
    else
      {
        observation_exit = read (observationDirectory + "-exit.json");
        observation_status = read (observationDirectory + "/status.json");
        ir = read (observationDirectory + "/compiler-architecture-ir.json");
        receipt = read (observationDirectory + "/project-architecture-receipt.json");
        replay = read "replay.json";
      }
  );
  result = import ./boundary-observation.nix {
    inherit
      case
      policy
      selection
      observed
      ;
  };
in
if result.valid then
  "boundary control ${case.id}: passed (not M1 acceptance)"
else
  throw ("boundary control ${case.id}: " + builtins.concatStringsSep ", " result.diagnostics)
