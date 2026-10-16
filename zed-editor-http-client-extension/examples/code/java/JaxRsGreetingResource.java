// JAX-RS: Quarkus, Jakarta EE, Helidon, Dropwizard.
// Click "🌐 Generate .http request" above a method, or press cmd+. on the line.
package com.example.jaxrs;

import jakarta.annotation.security.RolesAllowed;
import jakarta.ws.rs.*;
import jakarta.validation.Valid;

@Path("/greetings")
public class JaxRsGreetingResource {

    @GET
    public String all(@QueryParam("locale") String locale) { return "[]"; }

    @GET
    @Path("/{id}")
    public String one(@PathParam("id") long id, @HeaderParam("X-Trace-Id") String trace) { return "{}"; }

    @POST
    @RolesAllowed("admin")
    public String create(@Valid Greeting greeting) { return "{}"; }

    @DELETE
    @Path("/{id}")
    public void delete(@PathParam("id") long id) {}
}

class Greeting {
    private String message;
    private String language;
    private boolean formal;
}
