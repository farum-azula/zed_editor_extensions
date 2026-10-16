// Retrofit (Kotlin).
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
package com.example.retrofit

import retrofit2.http.*

interface RetrofitUserApi {

    @GET("/users/{id}")
    suspend fun get(@Path("id") id: String, @Query("expand") expand: Boolean): User

    @POST("/users")
    suspend fun create(@Header("X-Request-Id") rid: String, @Body user: User): User
}

data class User(val name: String, val email: String, val active: Boolean)
