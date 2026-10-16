# Faraday / HTTParty.
# Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
require "faraday"
require "httparty"

def questions
  HTTParty.get("https://api.stackexchange.com/2.2/questions")
end

def create_issue(conn)
  conn.post("/repos/demo/issues", body: { title: "bug", body: "steps" }, headers: { "X-Request-Id" => "demo" })
end

def issues(conn)
  conn.get("/repos/demo/issues", query: { state: "open" })
end
