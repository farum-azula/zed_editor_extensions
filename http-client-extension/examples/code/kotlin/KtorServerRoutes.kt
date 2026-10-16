// Ktor server routing.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
package com.example.ktor

import io.ktor.server.application.*
import io.ktor.server.request.*
import io.ktor.server.routing.*

fun Route.userRoutes() {
    get("/users/{id}") {
        val trace = call.request.headers["X-Trace-Id"]
    }

    get("/users") {
        val page = call.request.queryParameters["page"]
    }

    post("/users") {
        val user = call.receive<NewUser>()
    }

    delete("/users/{id}") {
    }
}

data class NewUser(val login: String, val email: String, val age: Int)
