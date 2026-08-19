/**
 * Quick-create command: a Form to create a todo, optionally plan for today.
 * No-view command — runs the mutation and shows a toast.
 */
import { Form, Action, ActionPanel, Toast, popToRoot, showToast } from "@raycast/api";
import { useState } from "react";
import { createTodo, planForToday, GraphqlError } from "./lib/graphql";

interface FormValues {
  title: string;
  description: string;
  planToday: boolean;
}

export default function QuickCreateCommand() {
  const [titleError, setTitleError] = useState<string | undefined>();
  const [submitting, setSubmitting] = useState(false);

  async function submit(values: FormValues) {
    const trimmed = values.title.trim();
    if (!trimmed) {
      setTitleError("Title is required");
      return;
    }
    setSubmitting(true);
    const toast = await showToast({
      style: Toast.Style.Animated,
      title: "Creating todo",
    });
    try {
      const todo = await createTodo(trimmed, values.description.trim() || undefined);
      if (values.planToday) {
        await planForToday(todo.id);
        toast.title = "Created and planned for today";
      } else {
        toast.title = "Created todo";
      }
      toast.style = Toast.Style.Success;
      toast.message = todo.title;
      await popToRoot();
    } catch (e: unknown) {
      toast.style = Toast.Style.Failure;
      if (e instanceof GraphqlError) {
        toast.title = "Could not create todo";
        toast.message = e.message;
      } else if (e instanceof TypeError) {
        toast.title = "Backend unreachable";
        toast.message = "Is the preflight server running on 127.0.0.1:8000?";
      } else {
        toast.title = "Failed";
        toast.message = e instanceof Error ? e.message : String(e);
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Form
      actions={
        <ActionPanel>
          <Action.SubmitForm title="Create Todo" onSubmit={submit} />
        </ActionPanel>
      }
      isLoading={submitting}
    >
      <Form.TextField
        id="title"
        title="Title"
        placeholder="What needs to happen?"
        error={titleError}
        onChange={() => {
          if (titleError) setTitleError(undefined);
        }}
      />
      <Form.TextArea id="description" title="Description" placeholder="Optional detail" enableMarkdown />
      <Form.Checkbox id="planToday" title="Plan for today" label="Add to today's plan" defaultValue />
    </Form>
  );
}
