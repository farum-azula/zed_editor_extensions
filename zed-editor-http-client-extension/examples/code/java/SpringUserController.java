// Spring Web (MVC / WebFlux). The class-level @RequestMapping prefix is included.
// Click "🌐 Generate .http request" above a mapping, or press cmd+. on the line.
package com.example.spring;

import java.util.List;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/users")
public class SpringUserController {

    @GetMapping
    public List<User> list(@RequestParam(defaultValue = "0") int page,
                           @RequestParam(value = "q", required = false) String query) { return List.of(); }

    @GetMapping("/{id}")
    public User get(@PathVariable long id) { return null; }

    @PostMapping
    @PreAuthorize("hasRole('ADMIN')")
    public User create(@RequestHeader("X-Request-Id") String requestId, @RequestBody CreateUser body) { return null; }

    @PutMapping("/{id}")
    public User update(@PathVariable long id, @RequestBody CreateUser body) { return null; }

    @RequestMapping(value = "/{id}/avatar", method = RequestMethod.DELETE)
    public void deleteAvatar(@PathVariable long id) {}
}

record CreateUser(String name, String email, int age, List<String> roles, Address address) {}

class Address {
    private String city;
    private String zip;
}
