// Read a committed fixture (`fixtures/<set>/<Type>/<name>.json`) and parse
// it with a schema, so component tests render real protocol values.

@module("node:fs") external readFileSync: (string, string) => string = "readFileSync"
@module("node:fs") external readdirSync: string => array<string> = "readdirSync"
@module("node:path") external join3: (string, string, string) => string = "join"
@module("node:path") external join4: (string, string, string, string) => string = "join"
@val external dirname: string = "import.meta.dirname"

let root = join3(dirname, "..", "..")

let json = (set: string, type_: string, name: string): JSON.t =>
  readFileSync(join4(root, "fixtures", set, type_ ++ "/" ++ name ++ ".json"), "utf8")
  ->JSON.parseOrThrow

let parse = (schema: S.t<'a>, set: string, type_: string, name: string): 'a =>
  S.parseJsonOrThrow(json(set, type_, name), schema)

let variants = (set: string, type_: string): array<string> =>
  readdirSync(join4(root, "fixtures", set, type_))
  ->Array.filter(f => String.endsWith(f, ".json"))
  ->Array.map(f => String.slice(f, ~start=0, ~end=String.length(f) - 5))
