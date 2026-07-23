# User CRUD API

[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/Axum-0.7-green)](https://github.com/tokio-rs/axum)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16-blue)](https://www.postgresql.org/)
[![Redis](https://img.shields.io/badge/Redis-7-red)](https://redis.io/)
[![Docker](https://img.shields.io/badge/Docker-Ready-blue)](https://www.docker.com/)
[![License](https://img.shields.io/badge/License-MIT-yellow)](https://opensource.org/licenses/MIT)

A production-ready Rust API with PostgreSQL, Redis, and JWT authentication.

## Features

- **Axum** - Modern, high-performance web framework
- **PostgreSQL** - Robust relational database
- **Redis** - Caching and session management
- **SQLx** - Async database driver
- **JWT Authentication** - Secure token-based auth with refresh tokens
- **Docker Compose** - Containerized development and deployment
- **API Versioning** - `/api/v1/` prefix for clean API evolution

## Architecture Diagrams

### System Architecture

```mermaid
flowchart TB
    subgraph Clients["Internet Clients"]
        Browser["Web Browser"]
        MobileApp["Mobile App"]
        ApiClients["API Clients"]
    end

    subgraph Gateway["API Gateway"]
        TLS["TLS Termination"]
    end

    subgraph RustAPI["Rust API (Axum)"]
        MW["Middleware<br/>CORS, Logging"]
        
        subgraph Routes["API Routes"]
            Auth["/auth/*"]
            Users["/users/*"]
            Tasks["/tasks/*"]
        end
        
        subgraph Services["Services"]
            AuthS["AuthService"]
            UserS["UserService"]
            TaskS["TaskService"]
        end
        
        MW --> Routes --> Services
        Services --> Repo["Repository"]
        Services --> Cache["Cache"]
    end

    subgraph DataStore["Data Stores"]
        PG[("PostgreSQL")]
        RD[("Redis")]
    end

    Clients --> TLS --> MW
    Repo --> PG
    Cache --> RD
```

### Request Flow

```mermaid
sequenceDiagram
    autonumber
    participant C as Client
    participant API as Axum
    participant Svc as Services
    participant Repo as Repository
    participant PG as PostgreSQL
    participant Cache as Redis

    C->>API: POST /api/v1/auth/register
    API->>Svc: user_data
    Svc->>Repo: Hash password
    Repo->>PG: INSERT user
    PG-->>Repo: user created
    Repo-->>Svc: user object
    Svc->>Cache: invalidate("users:list:*")
    Cache-->>Svc: OK
    Svc->>API: Token response
    API-->>C: 201 Created + JWT token

    Note over C: Use token for protected routes

    C->>API: GET /api/v1/users
    API->>Svc: token
    Svc->>Cache: get("users:list:1:100")
    
    alt Cache Hit
        Cache-->>Svc: cached data
    else Cache Miss
        Svc->>Repo: fetch users
        Repo->>PG: SELECT users
        PG-->>Repo: user list
        Repo-->>Svc: user list
        Svc->>Cache: setex(..., ttl)
    end

    Svc-->>API: paginated response
    API-->>C: 200 OK
```

### Database Schema

```mermaid
erDiagram
    USERS {
        uuid id PK "Primary Key"
        string name "Required"
        string email UK "Unique"
        int age "Optional"
        bool is_active "Default: true"
        bool is_superuser "Default: false"
        string hashed_password "Required"
        timestamp created_at "Auto"
        timestamp updated_at "Auto"
    }

    TASKS {
        uuid id PK "Primary Key"
        string title "Required"
        string description "Optional"
        int priority "1-5, Default: 1"
        bool completed "Default: false"
        uuid user_id FK "References Users"
        timestamp created_at "Auto"
        timestamp updated_at "Auto"
    }

    USERS ||--o{ TASKS : "creates"
```

### Caching Strategy

```mermaid
flowchart LR
    subgraph ReadOps["Read Operations"]
        A["GET /users<br/>/tasks/{id}"] --> B{Redis Cache}
        B -->|HIT| C["Return Cached"]
        B -->|MISS| D["Query PostgreSQL"]
        D --> E["Store in Redis<br/>TTL: 300s"]
        E --> C
    end

    subgraph WriteOps["Write Operations"]
        F["POST/PATCH/DELETE"] --> G["Write to PostgreSQL"]
        G --> H["Invalidate Cache"]
        H --> I["Return Response"]
    end
```

### Docker Architecture

```mermaid
graph TB
    subgraph Compose["Docker Compose"]
        subgraph Services["Services"]
            API["Rust API<br/>:8000"]
            PG["PostgreSQL<br/>:5432"]
            RD["Redis<br/>:6379"]
        end
        
        API --> PG
        API --> RD
    end

    subgraph Clients["Clients"]
        Browser["Browser"]
        Mobile["Mobile App"]
        Client["API Clients"]
    end

    Browser --> API
    Mobile --> API
    Client --> API
```

## Quick Start

### Prerequisites

- Rust 1.78+
- Docker & Docker Compose

### Local Development

```bash
# Clone the repository
git clone https://github.com/anqorithm/user-crud-api-rs
cd user-crud-api-rs

# Build
cargo build --release

# Start infrastructure with Docker
docker compose up -d postgres redis

# Run
cargo run
```

The API will be available at:
- **API**: http://localhost:8000
- **Docs**: http://localhost:8000/docs (Swagger)

### Using Docker (Full Stack)

```bash
# Build and start all services
docker compose up --build

# Stop all services
docker compose down
```

## Project Structure

```
src/
├── main.rs                 # Application entry point
├── config.rs                # Configuration
├── error.rs                 # Error handling
├── models.rs                # Data models
├── db/
│   └── mod.rs              # Database setup
├── handlers/
│   ├── auth.rs             # Authentication handlers
│   ├── users.rs            # User handlers
│   └── tasks.rs            # Task handlers
├── middleware/
│   └── auth.rs             # JWT middleware
└── services/
    ├── auth.rs             # Auth service
    ├── user.rs             # User service
    └── task.rs             # Task service
```

## Configuration

All configuration is managed through environment variables.

```env
APP__DATABASE_URL=postgres://postgres:postgres@localhost:5432/users_db
APP__REDIS_URL=redis://localhost:6379
APP__JWT_SECRET=your-super-secret-key-change-in-production
APP__JWT_EXPIRATION_HOURS=24
APP__JWT_REFRESH_EXPIRATION_DAYS=7
```

## API Endpoints

### Authentication

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/v1/auth/register` | Register new user |
| `POST` | `/api/v1/auth/login` | Login and get tokens |
| `POST` | `/api/v1/auth/refresh` | Refresh token |

### Users

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/v1/users` | List all users |
| `GET` | `/api/v1/users/{id}` | Get user by ID |
| `POST` | `/api/v1/users` | Create user |
| `PATCH` | `/api/v1/users/{id}` | Update user |
| `DELETE` | `/api/v1/users/{id}` | Delete user |

### Tasks

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/v1/tasks` | List tasks |
| `GET` | `/api/v1/tasks/{id}` | Get task by ID |
| `POST` | `/api/v1/tasks` | Create task |
| `PATCH` | `/api/v1/tasks/{id}` | Update task |
| `DELETE` | `/api/v1/tasks/{id}` | Delete task |

### Health

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check |

## License

MIT License - see LICENSE file for details.

---

**Built with Rust, PostgreSQL, Redis**