(identifier) @variable

(dotted_identifier_list) @string

; Methods
; --------------------
; `=> Book(...)`: a class after `=>`; a variable there (`=> heard.add(...)`) stays a variable.
((function_expression_body
  (identifier) @type)
  (#match? @type "^_?[A-Z]"))

; ((identifier)(selector (argument_part)) @function)
(((identifier) @function
  (#match? @function "^_?[a-z]"))
  .
  (selector
    .
    (argument_part))) @function

; Operators and Tokens
; --------------------
(template_substitution
  "$" @punctuation.special
  "{" @punctuation.special
  "}" @punctuation.special) @none

(template_substitution
  "$" @punctuation.special
  (identifier_dollar_escaped) @variable) @none

(escape_sequence) @string.escape

[
  "=>"
  ".."
  "??"
  ; null-aware access, cascade and assignment, and the null check `x!`
  "?."
  "?.."
  "??="
  "!"
  "=="
  "?"
  ":"
  "&&"
  "%"
  "<"
  ">"
  "="
  ">="
  "<="
  "||"
  ; compound assignment
  "+="
  "-="
  "*="
  "/="
  "~/="
  "%="
  "<<="
  ">>="
  ">>>="
  "&="
  "^="
  "|="
  ; bitwise, shifts and spread
  "&"
  "|"
  "^"
  "..."
  (bitwise_operator)
  (shift_operator)
  (multiplicative_operator)
  (increment_operator)
  (is_operator)
  (prefix_operator)
  (equality_operator)
  (additive_operator)
] @operator

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

; Delimiters
; --------------------
[
  ";"
  "."
  ","
] @punctuation.delimiter

; Types
; --------------------
(class_definition
  name: (identifier) @type)

(constructor_signature
  name: (identifier) @type)

(scoped_identifier
  scope: (identifier) @type)

(function_signature
  name: (identifier) @function.method)

; A factory constructor's own name, as a method's: `factory neonCat()`, `factory Old.named()`.
; `factory Old()` and the `Old` of `Old.named` stay types.
((factory_constructor_signature
  (identifier) @function.method
  .
  (formal_parameter_list))
  (#match? @function.method "^_?[a-z]"))

(enum_declaration
  name: (identifier) @type)

(enum_constant
  name: (identifier) @property)

((scoped_identifier
  scope: (identifier) @type
  name: (identifier) @type)
  (#match? @type "^[a-zA-Z]"))

(type_identifier) @type

; An import prefix naming a type or a constructor (`ui.Color`); an annotation's stays the
; annotation's (below).
((scoped_identifier
  scope: (identifier) @namespace)
  (#match? @namespace "^_?[a-z]"))

; A typedef's name, not the type it stands for (`Map` of `typedef Json = Map<String, Object?>`).
(type_alias
  .
  (type_identifier) @type.definition)

(mixin_declaration
  (identifier) @type)

; Variables
; --------------------
; var keyword
(inferred_type) @keyword

; A capitalised name is a type (`Status`, a single letter `E`), but not an all-caps constant (`PI`).
((identifier) @type
  (#match? @type "^_?[A-Z]([A-Za-z0-9_$]*[a-z][A-Za-z0-9_$]*)?$"))

; Annotations
; --------------------
(annotation
  "@" @attribute
  name: (identifier) @attribute)

; A qualified name: an import prefix (`@meta.immutable`, `@web.Get()`) is part of the annotation.
(annotation
  "@" @attribute
  name: (scoped_identifier
    scope: (identifier) @attribute
    name: (identifier) @attribute))

; A named constructor (`@RequestParam.of(...)`): the class as an annotation, the constructor as
; one called anywhere (`EdgeInsets.all(8)`).
((annotation
  name: (scoped_identifier
    scope: (identifier) @attribute
    name: (identifier) @function.method))
  (#match? @attribute "^_?[A-Z]"))

; Both (`@web.RequestParam.of(...)`).
(annotation
  "@" @attribute
  name: (scoped_identifier
    scope: (scoped_identifier
      scope: (identifier) @attribute
      name: (identifier) @attribute)
    name: (identifier) @function.method))

; properties
(unconditional_assignable_selector
  (identifier) @property)

(conditional_assignable_selector
  (identifier) @property)

(cascade_selector
  (identifier) @property)

(getter_signature
  (identifier) @property)

(setter_signature
  name: (identifier) @property)

((selector
  (unconditional_assignable_selector
    (identifier) @function.method))
  .
  (selector
    (argument_part
      (arguments))))

((selector
  (conditional_assignable_selector
    (identifier) @function.method))
  .
  (selector
    (argument_part
      (arguments))))

(cascade_section
  (cascade_selector
    (identifier) @function.method)
  .
  (argument_part
    (arguments)))

; Dot shorthand
(dot_shorthand
  (identifier) @property)

((dot_shorthand
  (identifier) @function.method)
  .
  (selector
    (argument_part
      (arguments))))

((dot_shorthand
  (identifier) @function.method)
  .
  (arguments))

; Some methods do not have a selector as a parent of the conditional_assignable_selector
; For example, super methods.
((unconditional_assignable_selector
  (identifier) @function.method)
  .
  (selector
    (argument_part
      (arguments))))

((conditional_assignable_selector
  (identifier) @function.method)
  .
  (selector
    (argument_part
      (arguments))))

; assignments
(assignment_expression
  left: (assignable_expression) @variable)

; A field or a top-level variable where it is declared, in the colour of its uses (`books.size`,
; and with semantic tokens, a bare `size`): `final int size;`, `static const name = '';`,
; `final events = <String>[];` at the top of a file. Not the initializer (`= other`).
(declaration
  (initialized_identifier_list
    (initialized_identifier
      .
      (identifier) @property)))

(declaration
  (static_final_declaration_list
    (static_final_declaration
      .
      (identifier) @property)))

(declaration
  (identifier_list
    (identifier) @property))

(program
  (initialized_identifier_list
    (initialized_identifier
      .
      (identifier) @property)))

(program
  (static_final_declaration_list
    (static_final_declaration
      .
      (identifier) @property)))

(program
  (identifier_list
    (identifier) @property))

(this) @variable.builtin

; A record's named fields, in the colour of their uses (`point.x`): `({int x, int y})`, `(x: 1)`,
; and a field an object pattern matches: `Point(x: var px)`.
(record_type_named_field
  (typed_identifier
    (identifier) @property))

(record_field
  (label
    (identifier) @property))

(object_pattern
  (identifier) @property)

; An import prefix: `as ui`, and `ui.Color` where a type is named. In an expression (`ui.window`) it
; cannot be told from a variable here; with semantic tokens it is (importPrefix).
(import_specification
  "as"
  .
  (identifier) @namespace)

(_
  (type_identifier) @namespace
  .
  "."
  .
  (type_identifier))

; A class through an import prefix: `ui.Color(0)`, `lazy.Foo.named()`; an all-caps constant
; (`Colors.RED`) stays one.
((selector
  (unconditional_assignable_selector
    (identifier) @type))
  (#match? @type "^_?[A-Z]([A-Za-z0-9_$]*[a-z][A-Za-z0-9_$]*)?$"))

; Parameters
; --------------------
(formal_parameter
  (identifier) @variable.parameter)

(named_argument
  (label
    (identifier) @variable.parameter))

; Literals
; --------------------
[
  (hex_integer_literal)
  (decimal_integer_literal)
  (decimal_floating_point_literal)
  ; TODO: inaccessible nodes
  ; (octal_integer_literal)
  ; (hex_floating_point_literal)
] @number

(symbol_literal) @string.special.symbol

(string_literal) @string

(true) @boolean

(false) @boolean

(null_literal) @constant.builtin

(comment) @comment

(documentation_comment) @comment.documentation

; Keywords
; --------------------
[
  "import"
  "library"
  "export"
  "as"
  "show"
  "hide"
] @keyword.import

; Reserved words (cannot be used as identifiers)
[
  (case_builtin)
  (void_type)
  "late"
  "required"
  "extension"
  "on"
  "class"
  "enum"
  "extends"
  "in"
  "is"
  "new"
  "super"
  "with"
  "Function"
] @keyword.definition

"return" @keyword.return

; Built in identifiers:
; alone these are marked as keywords
[
  (part_of_builtin)
  "deferred"
  "factory"
  "implements"
  "interface"
  "mixin"
  "part"
  "typedef"
  (assert_builtin)
] @keyword

; `get`, `set` and `operator` name variables too (`final set = <int>{};`, `get = 1;`): keywords only
; where they declare.
(getter_signature
  "get" @keyword)

(setter_signature
  "set" @keyword)

(operator_signature
  "operator" @keyword)

; `extension type Meters(int value)`
(extension_type_declaration
  "type" @keyword.definition)

; `Meters.new`: a constructor's tear-off
((unconditional_assignable_selector
  (identifier) @keyword)
  (#eq? @keyword "new"))

[
  "async"
  "async*"
  "sync*"
  "await"
  "yield"
] @keyword.coroutine

[
  (const_builtin)
  (final_builtin)
  "abstract"
  "covariant"
  "external"
  "static"
  "final"
  "base"
  "sealed"
] @keyword.modifier ; could be @type.qualifier

; A closure's parameters (`(alert) => ...`, `(String line) {...}`, `({a}) => ...`) apart from those
; of functions and methods.
(function_expression
  (formal_parameter_list
    (formal_parameter
      (identifier) @variable.parameter.closure)))

(function_expression
  (formal_parameter_list
    (optional_formal_parameters
      (formal_parameter
        (identifier) @variable.parameter.closure))))

[
  "if"
  "else"
  "switch"
  "default"
  "case"
  "when"
] @keyword.conditional

[
  "try"
  "throw"
  "catch"
  "finally"
  (rethrow_builtin)
] @keyword.exception

[
  "do"
  "while"
  "continue"
  (break_builtin)
  "for"
] @keyword.repeat

; A statement's label, where it is declared and where `break` or `continue` names it.
(labeled_statement
  (identifier) @label)

(break_statement
  (identifier) @label)

(continue_statement
  (identifier) @label)
