((request
  method: (method) @run @method
  url: (target_url) @url)
  (#set! tag http-request))

((request
  .
  url: (target_url) @run @url)
  (#set! tag http-request))
