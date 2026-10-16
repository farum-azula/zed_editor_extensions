(method) @keyword

(request
  url: (_) @link_uri)

(header
  name: (_) @property)

(header
  ":" @punctuation.delimiter)

(header
  value: (_) @string)

(variable_declaration
  name: (identifier) @variable)

(variable_declaration
  "=" @operator)

(variable
  name: (identifier) @variable.special)

[
  "{{"
  "}}"
] @punctuation.special

(http_version) @constant

(status_code) @number

(status_text) @string

(external_body
  path: (_) @string.special)

(comment) @comment

(comment
  name: (_) @attribute)

(request_separator) @comment.doc
