# Rails routes.
# Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
Rails.application.routes.draw do
  get "photos/:id", to: "photos#show"
  get "photos", to: "photos#index"
  post "/photos", to: "photos#create"
  patch "/photos/:id", to: "photos#update"
  delete "/photos/:id", to: "photos#destroy"
end
