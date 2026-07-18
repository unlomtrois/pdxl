; Highlights for PDXScript via tree-sitter-paradox.
; Node names come from the grammar's src/node-types.json; patterns are ordered
; specific -> general (tree-sitter gives earlier patterns priority).

; The keyword-like head of a condition / logical block (matches upstream).
(condition_statement (_) @keyword)
(logical_statement (_) @keyword)

; Assignment keys read as properties (`add_trait = brave` -> `add_trait`).
(assignment key: (identifier) @property)
(macro_map key: (identifier) @property)

(comment) @comment
(string) @string
(template_string) @string
(number) @number
(boolean) @boolean

; Everything else naming a value.
(identifier) @variable
