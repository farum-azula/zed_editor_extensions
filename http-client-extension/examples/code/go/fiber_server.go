// Fiber.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
package main

import "github.com/gofiber/fiber/v2"

type Ticket struct {
	Subject  string `json:"subject"`
	Priority int    `json:"priority"`
}

func fiberRoutes() {
	app := fiber.New()
	app.Get("/tickets/:id", getTicket)
	app.Post("/tickets", createTicket)
	app.Delete("/api/:id", deleteTicket)
}

func getTicket(c *fiber.Ctx) error {
	_ = c.Query("expand")
	_ = c.Get("X-Tenant")
	return nil
}

func createTicket(c *fiber.Ctx) error {
	var t Ticket
	c.BodyParser(&t)
	return nil
}

func deleteTicket(c *fiber.Ctx) error { return nil }
