(class_definition
  "class" @context
  name: (_) @name) @item

(mixin_declaration
  (mixin) @context
  .
  (identifier) @name) @item

(extension_declaration
  "extension" @context
  name: (_) @name) @item

(extension_type_declaration
  "extension" @context
  "type" @context
  name: (_) @name) @item

(enum_declaration
  "enum" @context
  name: (_) @name) @item

(enum_constant
  name: (_) @name) @item

(type_alias
  "typedef" @context
  .
  (type_identifier) @name) @item

; Constructors, named as they are called: `Old`, `Old.named`, `const Old.fixed`, `factory Old.of`.
(constructor_signature
  name: (_) @name
  ("." @name
    name: (_) @name)?) @item

(constant_constructor_signature
  (const_builtin) @context
  name: (_) @name
  ("." @name
    name: (_) @name)?) @item

(factory_constructor_signature
  "factory" @context
  .
  (identifier) @name
  ("." @name
    .
    (identifier) @name)?) @item

(redirecting_factory_constructor_signature
  "factory" @context
  .
  (identifier) @name
  ("." @name
    .
    (identifier) @name)?) @item

(function_signature
  name: (_) @name) @item

(getter_signature
  "get" @context
  name: (_) @name) @item

(setter_signature
  "set" @context
  name: (_) @name) @item

; Fields and top-level variables (not a local's second name, `final a = 1, b = 2;`).
(declaration
  (initialized_identifier_list
    (initialized_identifier
      .
      (identifier) @name) @item))

(program
  (initialized_identifier_list
    (initialized_identifier
      .
      (identifier) @name) @item))

(static_final_declaration
  .
  (identifier) @name) @item
