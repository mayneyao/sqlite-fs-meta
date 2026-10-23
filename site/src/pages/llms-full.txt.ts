import type { APIRoute } from "astro";
import { fullAgentDocumentation } from "../data/agent-docs";

export const GET: APIRoute = () =>
  new Response(fullAgentDocumentation, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
