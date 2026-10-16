// net/http server (Go 1.22 routing patterns) and client.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
package main

import (
	"encoding/json"
	"net/http"
)

type Order struct {
	CustomerID int      `json:"customer_id"`
	Items      []string `json:"items"`
	Express    bool     `json:"express"`
	Note       string   `json:"note,omitempty"`
}

func main() {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /health", health)
	mux.HandleFunc("POST /orders", createOrder)
	mux.HandleFunc("DELETE /orders/{id}", deleteOrder)
	http.ListenAndServe(":8080", mux)
}

func health(w http.ResponseWriter, r *http.Request) {}

func createOrder(w http.ResponseWriter, r *http.Request) {
	var o Order
	json.NewDecoder(r.Body).Decode(&o)
	_ = r.URL.Query().Get("dry_run")
	_ = r.Header.Get("Idempotency-Key")
}

func deleteOrder(w http.ResponseWriter, r *http.Request) {
	_ = r.Header.Get("Authorization")
}

func fetchOrder(id string) (*http.Response, error) {
	req, _ := http.NewRequest(http.MethodGet, "https://api.example.com/v1/orders/"+id, nil)
	return http.DefaultClient.Do(req)
}
