// ASP.NET Core controllers.
// Click "🌐 Generate .http request" above a route, or press cmd+. on the line.
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;

namespace Example.Controllers;

public record CreateUser(string Name, string Email, int Age, bool Admin);

[Route("api/[controller]")]
public class UsersController : ControllerBase
{
    [HttpGet("{id}")]
    public User Get(int id, [FromQuery] bool expand) => null;

    [HttpGet]
    public User[] List([FromQuery(Name = "page")] int page) => null;

    [HttpPost]
    [Authorize]
    public User Create([FromHeader(Name = "X-Request-Id")] string rid, [FromBody] CreateUser body) => null;

    [HttpDelete("{id}")]
    public void Delete(int id) {}
}
