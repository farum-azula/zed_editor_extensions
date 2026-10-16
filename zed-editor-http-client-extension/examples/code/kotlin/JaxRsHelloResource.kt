// JAX-RS (Quarkus Kotlin).
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
package com.example.quarkus

import jakarta.ws.rs.*

@Path("/hello")
class JaxRsHelloResource {

    @GET
    fun hello(@QueryParam("name") name: String): String = "hi"

    @POST
    @Path("/batch")
    fun batch(@HeaderParam("X-Tenant") tenant: String): String = "ok"
}
