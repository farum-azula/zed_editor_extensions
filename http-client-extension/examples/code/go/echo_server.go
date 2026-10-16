// Echo.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
package main

import "github.com/labstack/echo/v4"

type Comment struct {
	Author string `json:"author"`
	Text   string `json:"text"`
}

func echoRoutes() {
	e := echo.New()
	e.GET("/posts/:id/comments", listComments)
	e.POST("/posts/:id/comments", addComment)
	e.PUT("/comments/:id", updateComment)
}

func listComments(c echo.Context) error {
	_ = c.QueryParam("limit")
	return nil
}

func addComment(c echo.Context) error {
	var body Comment
	c.Bind(&body)
	return nil
}

func updateComment(c echo.Context) error { return nil }
