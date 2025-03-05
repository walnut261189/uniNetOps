# uniNetOps
A universal network operation automater toolkit supporting templatising daily automation activities.
Features:
1. Universal template for network device os upgrade

## Notes:
The code has been generalized to serve as a template for any vendor type and network device. It uses an abstraction via the NetworkDevice trait, which can be implemented for different vendor types. You can customize it further for specific device requirements or protocols (e.g., gRPC).

### Configuration Info
The code loads parameters from a config.json file, allowing dynamic runtime updates without restarting the application. The configuration file is parsed into a shared, mutable structure that can be updated in real-time using a watcher mechanism.

### Configuration Parameters
1. base_url: The API endpoint of the Cisco device.
2. token: The authentication token for the Cisco device's API.
3. os_file_path: Path to the Cisco OS upgrade binary file on your system.
#### Usage:
Place the config.json file in the root directory of your application.
The application will load these parameters dynamically at runtime.

## Usage instructions for the project
Run the following command to fetch and install all dependencies
```
  cargo build
```

```
config/development.json
{
  "server": {
    "port": 3000
  },
  "rabbitmq": {
    "host": "amqp://localhost",
    "queue": "dev_message_queue",
    "reconnectInterval": 3000
  },
  "logging": {
    "logDirectory": "./logs/development",
    "maxSize": "10m",
    "maxFiles": "1d"
  },
  "rateLimiter": {
    "windowMs": 60000,
    "maxRequests": 200
  },
  "auth": {
    "tokenSecret": "dev-secure-token-secret"
  }
}

config/production.json
{
  "server": {
    "port": 8080
  },
  "rabbitmq": {
    "host": "amqp://prod-rabbitmq-host",
    "queue": "prod_message_queue",
    "reconnectInterval": 10000
  },
  "logging": {
    "logDirectory": "./logs/production",
    "maxSize": "10m",
    "maxFiles": "7d"
  },
  "rateLimiter": {
    "windowMs": 60000,
    "maxRequests": 50
  },
  "auth": {
    "tokenSecret": "prod-secure-token-secret"
  }
}

controllers/message.controller.ts

import { Request, Response, NextFunction } from "express";
import { publishToQueue } from "../services/rabbitmq.service";
import { logger } from "../services/logger.service";
import { trackUnsentMessage } from "../services/messageQueue.service";

/**
 * Handles incoming message payload and publishes it to RabbitMQ.
 * Works asynchronously to handle high loads.
 */
export const publishMessage = async (req: Request, res: Response, next: NextFunction) => {
  try {
    const { body } = req;
    logger.info(`Received message: ${JSON.stringify(body)}`);

    // Validate payload
    if (!body || Object.keys(body).length === 0) {
      logger.warn("Invalid payload received");
      return res.status(400).json({ error: "Payload cannot be empty" });
    }

    // Attempt to publish the message
    const published = await publishToQueue(body);

    if (published) {
      return res.status(200).json({ message: "Message published successfully" });
    } else {
      // Store unsent messages for retry
      await trackUnsentMessage(body);
      return res.status(503).json({ error: "RabbitMQ unavailable, message stored for retry" });
    }
  } catch (error) {
    logger.error(`Error publishing message: ${error}`);
    next(error);
  }
};

middlewares/auth.middleware.ts
import { Request, Response, NextFunction } from "express";
import jwt from "jsonwebtoken";
import config from "../utils/config";
import { logger } from "../services/logger.service";

/**
 * Middleware to verify JWT token.
 */
export const authenticateToken = (req: Request, res: Response, next: NextFunction) => {
  const token = req.header("Authorization")?.split(" ")[1];

  if (!token) {
    logger.warn("Unauthorized access attempt - No token provided");
    return res.status(401).json({ error: "Access denied. No token provided." });
  }

  try {
    jwt.verify(token, config.JWT_SECRET);
    next();
  } catch (err) {
    logger.warn("Unauthorized access attempt - Invalid token");
    return res.status(403).json({ error: "Invalid token." });
  }
};

middlewares/rateLimiter.middleware.ts

import rateLimit from "express-rate-limit";

/**
 * Rate limiter to prevent excessive API requests.
 */
export const rateLimiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15 minutes
  max: 100, // Max 100 requests per windowMs
  message: "Too many requests, please try again later.",
  headers: true,
});

routes/message.routes.ts
import { Router } from "express";
import { publishMessage } from "../controllers/message.controller";
import { authenticateToken } from "../middlewares/auth.middleware";
import { rateLimiter } from "../middlewares/rateLimiter.middleware";

const router = Router();

/**
 * Routes for message publishing
 */
router.post("/publish", authenticateToken, rateLimiter, publishMessage);

export default router;


services/logger.service.ts

import winston from "winston";
import fs from "fs";
import path from "path";
import DailyRotateFile from "winston-daily-rotate-file";

// Log directory
const logDir = path.join(__dirname, "../../logs");

// Ensure the log directory exists
if (!fs.existsSync(logDir)) {
  fs.mkdirSync(logDir, { recursive: true });
}

// Function to get log file name based on hour range
const getLogFilename = () => {
  const now = new Date();
  const startHour = now.getHours();
  const endHour = startHour + 1;
  const date = now.toISOString().split("T")[0]; // YYYY-MM-DD format
  return `${date}-${startHour}-${endHour}.log`;
};

// Winston logger configuration
export const logger = winston.createLogger({
  level: "info",
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.printf(({ timestamp, level, message }) => {
      return `${timestamp} [${level.toUpperCase()}]: ${message}`;
    })
  ),
  transports: [
    new DailyRotateFile({
      filename: path.join(logDir, getLogFilename()),
      datePattern: "YYYY-MM-DD-HH",
      maxSize: "10m", // Max log file size of 10MB
      maxFiles: "24h", // Keep logs for 24 hours
      zippedArchive: true,
    }),
    new winston.transports.Console(),
  ],
});

// Function to log errors separately
export const errorLogger = winston.createLogger({
  level: "error",
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.json()
  ),
  transports: [
    new winston.transports.File({
      filename: path.join(logDir, "error.log"),
      maxsize: 10 * 1024 * 1024, // 10MB max size
    }),
    new winston.transports.Console(),
  ],
});

services/messageQueue.service.ts

import fs from "fs";
import path from "path";
import { logger } from "./logger.service";
import { RabbitMQService } from "./rabbitmq.service";

const unsentMessagesFile = path.join(__dirname, "../../logs/unsentMessages.json");

export class MessageQueueService {
  private static instance: MessageQueueService;
  private messages: any[] = [];

  private constructor() {
    this.loadUnsentMessages();
  }

  static getInstance(): MessageQueueService {
    if (!MessageQueueService.instance) {
      MessageQueueService.instance = new MessageQueueService();
    }
    return MessageQueueService.instance;
  }

  private loadUnsentMessages() {
    if (fs.existsSync(unsentMessagesFile)) {
      const data = fs.readFileSync(unsentMessagesFile, "utf-8");
      this.messages = JSON.parse(data);
    }
  }

  private saveUnsentMessages() {
    fs.writeFileSync(unsentMessagesFile, JSON.stringify(this.messages, null, 2));
  }

  addMessage(message: any) {
    this.messages.push(message);
    this.saveUnsentMessages();
  }

  async retryUnsentMessages() {
    const rabbitMQService = RabbitMQService.getInstance();
    for (const msg of this.messages) {
      const success = await rabbitMQService.publishMessage(msg);
      if (success) {
        this.messages = this.messages.filter((m) => m !== msg);
        this.saveUnsentMessages();
      }
    }
  }
}

services/rabbitmq.service.ts

import amqp, { Connection, Channel } from "amqplib";
import { logger, errorLogger } from "./logger.service";
import { MessageQueueService } from "./messageQueue.service";
import CircuitBreaker from "../utils/circuitBreaker";

export class RabbitMQService {
  private static instance: RabbitMQService;
  private connection: Connection | null = null;
  private channel: Channel | null = null;
  private queue = "messageQueue";

  private constructor() {}

  static getInstance(): RabbitMQService {
    if (!RabbitMQService.instance) {
      RabbitMQService.instance = new RabbitMQService();
    }
    return RabbitMQService.instance;
  }

  async connect() {
    try {
      this.connection = await amqp.connect("amqp://localhost");
      this.channel = await this.connection.createChannel();
      await this.channel.assertQueue(this.queue, { durable: true });
      logger.info("Connected to RabbitMQ");
    } catch (error) {
      errorLogger.error(`RabbitMQ connection error: ${error}`);
      setTimeout(() => this.connect(), 5000);
    }
  }

  async publishMessage(message: any): Promise<boolean> {
    try {
      if (!this.channel) throw new Error("RabbitMQ channel not available");

      const circuitBreaker = CircuitBreaker.getInstance();
      return await circuitBreaker.fire(async () => {
        this.channel!.sendToQueue(this.queue, Buffer.from(JSON.stringify(message)), { persistent: true });
        logger.info(`Message published: ${JSON.stringify(message)}`);
        return true;
      });
    } catch (error) {
      errorLogger.error(`Message publish failed: ${error}`);
      MessageQueueService.getInstance().addMessage(message);
      return false;
    }
  }
}

utils/circuitBreaker.ts

import CircuitBreaker from "opossum";

export default class CircuitBreakerService {
  private static instance: CircuitBreaker;

  private constructor() {}

  static getInstance(): CircuitBreaker {
    if (!CircuitBreakerService.instance) {
      CircuitBreakerService.instance = new CircuitBreaker(
        async (action: Function) => action(),
        {
          timeout: 5000,
          errorThresholdPercentage: 50,
          resetTimeout: 10000,
        }
      );
    }
    return CircuitBreakerService.instance;
  }
}

utils/config.ts
import * as dotenv from "dotenv";
import config from "config";

dotenv.config();

export const AppConfig = {
  port: process.env.PORT || config.get("server.port"),
  jwtSecret: process.env.JWT_SECRET || config.get("auth.jwtSecret"),
  rabbitMQUrl: process.env.RABBITMQ_URL || config.get("rabbitmq.url"),
};


src/app.ts
import express from "express";
import bodyParser from "body-parser";
import cors from "cors";
import helmet from "helmet";
import rateLimiter from "./middlewares/rateLimiter.middleware";
import messageRoutes from "./routes/message.routes";
import { logger } from "./services/logger.service";

const app = express();

app.use(helmet());
app.use(cors());
app.use(bodyParser.json());
app.use(rateLimiter);

app.use("/api/messages", messageRoutes);

app.use((req, res) => {
  res.status(404).json({ error: "Not Found" });
});

app.use((err: any, req: any, res: any, next: any) => {
  logger.error(err.message);
  res.status(500).json({ error: "Internal Server Error" });
});

export default app;


src/server.ts

import app from "./app";
import { AppConfig } from "./utils/config";
import { RabbitMQService } from "./services/rabbitmq.service";

const PORT = AppConfig.port;

const startServer = async () => {
  await RabbitMQService.getInstance().connect();
  app.listen(PORT, () => {
    console.log(`Server running on port ${PORT}`);
  });
};

startServer();

swagger/swagger.yaml
openapi: 3.0.0
info:
  title: RabbitMQ Producer API
  description: API for publishing messages to RabbitMQ
  version: 1.0.0
servers:
  - url: http://localhost:3000/api
    description: Local server
paths:
  /messages/publish:
    post:
      summary: Publish a message
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                message:
                  type: string
                  example: "Hello, RabbitMQ!"
      responses:
        "200":
          description: Message published successfully
        "500":
          description: Internal server error


.env file

PORT=3000
JWT_SECRET=mysecretkey
RABBITMQ_URL=amqp://localhost


```










