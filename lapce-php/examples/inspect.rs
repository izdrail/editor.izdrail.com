fn main() {
    let source = "<h1>λ</h1><script>let x=1;</script><style>.a{color:red}</style><?php\n/** @var User $u */\n$sql = 'SELECT * FROM users';\n$x=<<<SQL\nSELECT * FROM users\nSQL;\n$x=<<<'SQL'\nSELECT id FROM x\nSQL;\nclass X{function f(User $u): void { $u-> }} ?>end";
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lapce_php::language(false)).unwrap();
    let tree = parser.parse(source, None).unwrap();
    fn walk(node: tree_sitter::Node, source: &str, depth: usize) {
        println!(
            "{}{} {:?} {:?}",
            " ".repeat(depth),
            node.kind(),
            node.byte_range(),
            &source[node.byte_range()]
        );
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk(child, source, depth + 1)
        }
    }
    walk(tree.root_node(), source, 0);
}
