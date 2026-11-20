; Zed colours a node with the last pattern that captures it, and a node inside another with its
; own capture: patterns for whole lines and blocks come first, those for the tokens in them after.
; Only node types of the grammar pinned in extension.toml may appear here; one unknown node makes
; Zed drop the whole file (cli/tests/queries.rs checks every pattern against that grammar).

; Comments and request separators
(comment) @comment

(request_separator) @comment

(request_separator
  value: (_) @title)

; Directive comments: `# @name login`, `# @no-redirect`, `# @env dev`
(comment
  "@" @attribute
  name: (_) @attribute)

(comment
  "=" @operator)

(comment
  value: (_) @string.special)

((comment
  name: (_) @attribute
  value: (_) @label)
  (#eq? @attribute "name"))

; Requests
(request
  url: (_) @link_uri)

(method) @function.method

(http_version) @constant

; Headers
(header
  name: (_) @property)

(header
  ":" @punctuation.delimiter)

(header
  value: (_) @string)

; File variables: `@token = value`
(variable_declaration
  "@" @punctuation.special
  name: (_) @variable)

(variable_declaration
  "=" @operator)

(variable_declaration
  value: (_) @string)

; `{{variables}}`: user variables, the built-in `{{$uuid}}`, `{{$timestamp}}`…, and their braces
(variable
  name: (_) @variable.special)

((variable
  name: (_) @variable.builtin)
  (#match? @variable.builtin "^\\$"))

(variable
  [
    "{{"
    "}}"
  ] @punctuation.special)

; Responses
(status_code) @number

(status_text) @string

; Files: a body read from one (`< ./body.json`), a response written to one (`>> ./out.json`),
; handler scripts (`> ./handler.js`)
(external_body
  path: (_) @string.special)

(res_redirect
  path: (_) @string.special)

(pre_request_script
  (path) @string.special)

(res_handler_script
  (path) @string.special)

; Inline scripts: `> {% ... %}`
(script
  [
    "{%"
    "%}"
  ] @punctuation.special)
