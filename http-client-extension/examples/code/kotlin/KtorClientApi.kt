// Ktor HTTP client.
// Click "🌐 Generate .http request" above a call, or press cmd+. on the line.
package com.example.ktor

import io.ktor.client.*
import io.ktor.client.request.*

class KtorClientApi(private val client: HttpClient, private val page: Int) {

    suspend fun docs() = client.get("https://ktor.io/docs/$page")

    suspend fun createTask(task: Task) = client.post("/tasks") {
        setBody(task)
    }
}

data class Task(val title: String, val done: Boolean)
