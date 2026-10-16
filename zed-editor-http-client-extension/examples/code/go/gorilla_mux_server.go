// gorilla/mux.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
package main

import (
	"encoding/json"
	"net/http"

	"github.com/gorilla/mux"
)

type Product struct {
	SKU   string  `json:"sku"`
	Title string  `json:"title"`
	Price float64 `json:"price"`
}

func gorillaRoutes() {
	r := mux.NewRouter()
	r.HandleFunc("/products", listProducts).Methods("GET")
	r.HandleFunc("/products", createProduct).Methods("POST")
	r.HandleFunc("/products/{key}", updateProduct).Methods("PUT")
}

func listProducts(w http.ResponseWriter, r *http.Request) {
	_ = r.URL.Query().Get("category")
}

func createProduct(w http.ResponseWriter, r *http.Request) {
	var p Product
	json.NewDecoder(r.Body).Decode(&p)
	_ = r.Header.Get("X-Tenant")
}

func updateProduct(w http.ResponseWriter, r *http.Request) {}
