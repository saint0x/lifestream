import { useState } from "react";
import { ArrowRight, Check, Loader2 } from "lucide-react";
import { audienceSignupKind, type PageContent } from "@/content/pages";
import { submitSignup } from "@/lib/signup";

type FormState = Record<string, string>;

export function SignupForm({ page }: { readonly page: PageContent }) {
  const [values, setValues] = useState<FormState>({});
  const [state, setState] = useState<"idle" | "submitting" | "success" | "error">("idle");
  const [error, setError] = useState("");

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setState("submitting");
    setError("");

    try {
      await submitSignup({
        kind: audienceSignupKind(page.audience),
        audience: page.audience,
        name: values.name ?? "",
        email: values.email ?? "",
        company: values.company,
        website: values.website,
        budget: values.budget,
        message: values.message || values.audience,
        sourcePath: window.location.pathname,
      });
      setState("success");
      setValues({});
    } catch (caught) {
      setState("error");
      setError(caught instanceof Error ? caught.message : "Signup could not be submitted.");
    }
  }

  return (
    <form className="vl-form" onSubmit={handleSubmit}>
      <div>
        <span className="vl-label">Access</span>
        <h2>{page.formTitle}</h2>
        <p>{page.formSubtitle}</p>
      </div>

      <div className="vl-form__grid">
        {page.fields.map((field) => {
          const value = values[field.name] ?? "";
          const common = {
            id: field.name,
            name: field.name,
            value,
            required: field.required,
            placeholder: field.placeholder,
            onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) =>
              setValues((current) => ({ ...current, [field.name]: event.target.value })),
          };

          return (
            <label key={field.name} className={field.multiline ? "is-wide" : undefined} htmlFor={field.name}>
              <span>{field.label}</span>
              {field.multiline ? <textarea {...common} rows={4} /> : <input {...common} />}
            </label>
          );
        })}
      </div>

      {state === "error" ? <p className="vl-form__message is-error">{error}</p> : null}
      {state === "success" ? (
        <p className="vl-form__message"><Check size={15} /> {page.formSuccess}</p>
      ) : null}

      <button className="vl-button vl-button--primary" disabled={state === "submitting"} type="submit">
        {state === "submitting" ? <Loader2 className="is-spinning" size={16} /> : <ArrowRight size={16} />}
        {page.primaryCta}
      </button>
    </form>
  );
}
