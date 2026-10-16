// Gin.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
package main

import "github.com/gin-gonic/gin"

type Login struct {
	User     string `json:"user"`
	Password string `json:"password"`
}

func ginRoutes() {
	r := gin.Default()
	r.POST("/login", login)
	r.GET("/users/:id", getUser)
	r.DELETE("/users/:id", deleteUser)
}

func login(c *gin.Context) {
	var body Login
	c.ShouldBindJSON(&body)
	_ = c.GetHeader("X-Device-Id")
}

func getUser(c *gin.Context) {
	_ = c.Query("expand")
}

func deleteUser(c *gin.Context) {}
