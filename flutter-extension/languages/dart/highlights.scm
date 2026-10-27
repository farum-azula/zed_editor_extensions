(identifier) @variable

(dotted_identifier_list) @string

; Methods
; --------------------
(super) @function

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

(type_alias
  (type_identifier) @type.definition)

; Variables
; --------------------
; var keyword
(inferred_type) @keyword

((identifier) @type
  (#match? @type "^_?[A-Z].*[a-z]"))

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

(this) @variable.builtin

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
  "get"
  "implements"
  "interface"
  "library"
  "operator"
  "mixin"
  "part"
  "set"
  "typedef"
] @keyword

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

; when used as an identifier:
((identifier) @variable.builtin
  (#any-of? @variable.builtin
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))

; ...but one naming a method, a property or a parameter keeps that name's colour, like any other
; name: `ImportBatch import(...)`, `books.import(...)`, `settings.get`, `{bool set}`, `set: true`.
((function_signature
  name: (identifier) @function.method)
  (#any-of? @function.method
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((getter_signature
  (identifier) @property)
  (#any-of? @property
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((setter_signature
  name: (identifier) @property)
  (#any-of? @property
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((unconditional_assignable_selector
  (identifier) @property)
  (#any-of? @property
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((conditional_assignable_selector
  (identifier) @property)
  (#any-of? @property
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((cascade_selector
  (identifier) @property)
  (#any-of? @property
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((unconditional_assignable_selector
  (identifier) @function.method)
  .
  (selector
    (argument_part
      (arguments)))
  (#any-of? @function.method
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((conditional_assignable_selector
  (identifier) @function.method)
  .
  (selector
    (argument_part
      (arguments)))
  (#any-of? @function.method
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((selector
  (unconditional_assignable_selector
    (identifier) @function.method))
  .
  (selector
    (argument_part
      (arguments)))
  (#any-of? @function.method
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((selector
  (conditional_assignable_selector
    (identifier) @function.method))
  .
  (selector
    (argument_part
      (arguments)))
  (#any-of? @function.method
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((cascade_section
  (cascade_selector
    (identifier) @function.method)
  .
  (argument_part
    (arguments)))
  (#any-of? @function.method
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((formal_parameter
  (identifier) @variable.parameter)
  (#any-of? @variable.parameter
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))
((named_argument
  (label
    (identifier) @variable.parameter))
  (#any-of? @variable.parameter
    "abstract" "as" "covariant" "deferred" "dynamic" "export" "external" "factory" "Function" "get"
    "implements" "import" "interface" "library" "operator" "mixin" "part" "set" "static" "typedef"))

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
] @keyword.conditional

[
  "try"
  "throw"
  "catch"
  "finally"
  (rethrow_builtin)
  (break_statement)
] @keyword.exception

[
  "do"
  "while"
  "continue"
  "for"
] @keyword.repeat
