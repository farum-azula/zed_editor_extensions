// chi.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
package main

import (
	"encoding/json"
	"net/http"

	"github.com/go-chi/chi/v5"
)

type Article struct {
	Title string `json:"title"`
	Body  string `json:"body"`
	Draft bool   `json:"draft"`
}

func chiRoutes() {
	r := chi.NewRouter()
	r.Get("/articles/{articleID}", getArticle)
	r.Post("/articles", createArticle)
	r.Delete("/articles/{articleID}", deleteArticle)
}

func getArticle(w http.ResponseWriter, r *http.Request) {
	_ = r.URL.Query().Get("format")
}

func createArticle(w http.ResponseWriter, r *http.Request) {
	var a Article
	json.NewDecoder(r.Body).Decode(&a)
	_ = r.Header.Get("X-Author")
}

func deleteArticle(w http.ResponseWriter, r *http.Request) {}
