; The ▶ on every request. Zed keeps one runnable per line — the last match — so the patterns
; for requests with a `# @name` come last, and their task is labelled with the name.

((section
  (request
    method: (method) @run @method
    url: (target_url) @url))
  (#set! tag http-request))

((section
  (request
    .
    url: (target_url) @run @url))
  (#set! tag http-request))

((section
  (comment
    name: (identifier) @_directive
    value: (value) @name)
  (request
    method: (method) @run @method
    url: (target_url) @url))
  (#eq? @_directive "name")
  (#set! tag http-request-named))

((section
  (comment
    name: (identifier) @_directive
    value: (value) @name)
  (request
    .
    url: (target_url) @run @url))
  (#eq? @_directive "name")
  (#set! tag http-request-named))
