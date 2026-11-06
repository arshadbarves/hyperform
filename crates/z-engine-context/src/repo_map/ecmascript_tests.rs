use super::super::language::{describe, outline_for_test};

const TYPESCRIPT: &str = r#"import { x } from "./x";

export interface Shape {
  area(): number;
  name: string;
}

export type Id = string | number;

export enum Color { Red, Green }

export abstract class Base implements Shape {
  abstract area(): number;
  name = "base";
  onClick = () => {};
  describe(): string { const inner = 1; return this.name; }
}

export default function main(): void {}

export const MAX_SIZE = 10;
export const toId = (value: number): Id => value;
let counter = 0;

namespace Geometry {
  export function scale(factor: number) {}
}

declare module "left-pad" {
  export function pad(s: string): string;
}

declare function ambient(): void;
"#;

#[test]
fn outlines_typescript_declarations_and_members() {
    let symbols = outline_for_test("src/shapes.ts", TYPESCRIPT);
    assert_eq!(
        describe(&symbols),
        [
            "interface Shape L3",
            "  method area L4",
            "type Id L8",
            "enum Color L10",
            "class Base L12",
            "  method area L13",
            "  method onClick L15",
            "  method describe L16",
            "function main L19",
            "const MAX_SIZE L21",
            "function toId L22",
            "namespace Geometry L25",
            "  function scale L26",
            "module \"left-pad\" L29",
            "  function pad L30",
            "function ambient L33",
        ]
    );
    let module = symbols.iter().find(|s| s.kind == "module").unwrap();
    assert_eq!(module.ident, None);
}

#[test]
fn outlines_tsx_components() {
    let symbols = outline_for_test(
        "ui/App.tsx",
        "export const App = () => <div className=\"app\" />;\nexport function Header() { return <h1 />; }\n",
    );
    assert_eq!(
        describe(&symbols),
        ["function App L1", "function Header L2"]
    );
}

#[test]
fn outlines_javascript_including_commonjs_exports() {
    let source = r#"const path = require("path");
function* ids() {}
class Store {
  constructor() {}
  static create = function () {};
  get size() { return 0; }
}
module.exports.load = function (file) {};
exports.save = (file) => {};
var legacy = function () {};
var plain = 1;
const LIMIT = 5;
module.exports = function createStore() {};
module.exports = () => {};
"#;
    let symbols = outline_for_test("lib/store.cjs", source);
    assert_eq!(
        describe(&symbols),
        [
            "function ids L2",
            "class Store L3",
            "  method constructor L4",
            "  method create L5",
            "  method size L6",
            "function load L8",
            "function save L9",
            "function legacy L10",
            "const LIMIT L12",
            "function createStore L13",
        ]
    );
}
