import type { APIRoute, GetStaticPaths } from "astro";
import { agentDocuments, type AgentDocument } from "../../data/agent-docs";

export const getStaticPaths = (() =>
  Object.keys(agentDocuments).map((document) => ({
    params: { document },
  }))) satisfies GetStaticPaths;

export const GET: APIRoute = ({ params }) => {
  const document = params.document;
  if (!document || !Object.hasOwn(agentDocuments, document)) {
    return new Response("Not found", { status: 404 });
  }
  return new Response(agentDocuments[document as AgentDocument], {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
};
