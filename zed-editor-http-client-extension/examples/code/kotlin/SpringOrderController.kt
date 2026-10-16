// Spring Web (Kotlin).
// Click "🌐 Generate .http request" above a mapping, or press cmd+. on the line.
package com.example.spring

import org.springframework.security.access.prepost.PreAuthorize
import org.springframework.web.bind.annotation.*

@RestController
@RequestMapping("/api/orders")
class SpringOrderController {

    @GetMapping("/{id}")
    fun get(@PathVariable id: Long): Order? = null

    @GetMapping
    fun list(@RequestParam("status") status: String, @RequestParam page: Int): List<Order> = emptyList()

    @PostMapping
    @PreAuthorize("hasRole('USER')")
    fun create(@RequestHeader("X-Request-Id") rid: String, @RequestBody body: CreateOrder): Order? = null

    @DeleteMapping("/{id}")
    fun delete(@PathVariable id: Long) {}
}

data class CreateOrder(val customer: String, val total: Double, val items: List<String>, val express: Boolean)
