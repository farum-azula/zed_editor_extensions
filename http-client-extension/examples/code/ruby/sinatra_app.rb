# Sinatra.
# Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
require "sinatra"

get "/hello/:name" do
  params[:name]
end

get "/notes" do
  params["page"]
end

post "/notes" do
  params[:title]
end

delete "/notes/:id" do
  status 204
end
