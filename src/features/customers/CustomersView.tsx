import { FormEvent, useCallback, useEffect, useState } from "react";
import { money } from "../../lib/money";
import { selectNumericInput } from "../../lib/numberInput";
import { adjustCustomerCredit, listCustomers, upsertCustomer } from "../../lib/posApi";
import type { Customer } from "../../types";
import { CustomerCreditModal, type CustomerCreditDraft } from "./CustomerModals";

export function CustomersView({ showToast }: { showToast: (message: string) => void }) {
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [form, setForm] = useState({ name: "", phone: "", email: "", credit_limit: 0, id: undefined as number | undefined });
  const [creditDraft, setCreditDraft] = useState<CustomerCreditDraft | null>(null);
  const editing = Boolean(form.id);
  const resetForm = () => setForm({ name: "", phone: "", email: "", credit_limit: 0, id: undefined });

  const refresh = useCallback(async () => {
    setCustomers(await listCustomers());
  }, []);

  useEffect(() => {
    refresh().catch((error) => showToast(String(error)));
  }, [refresh, showToast]);

  const save = async (event: FormEvent) => {
    event.preventDefault();
    try {
      await upsertCustomer(form);
      resetForm();
      await refresh();
      showToast("Cliente guardado");
    } catch (error) {
      showToast(String(error));
    }
  };

  const saveCredit = async (amount: number, reason: string, paymentMethod?: "cash" | "card" | "transfer") => {
    if (!creditDraft) return;
    const signedAmount = creditDraft.mode === "charge" ? amount : -amount;
    const nextBalance = creditDraft.customer.balance + signedAmount;
    if (creditDraft.mode === "charge" && creditDraft.customer.credit_limit <= 0) {
      showToast("Cliente sin limite de credito");
      return;
    }
    if (creditDraft.mode === "charge" && nextBalance > creditDraft.customer.credit_limit) {
      showToast("Cargo supera limite de credito");
      return;
    }
    if (creditDraft.mode === "payment" && nextBalance < 0) {
      showToast("Abono mayor al saldo");
      return;
    }
    try {
      await adjustCustomerCredit({
        customer_id: creditDraft.customer.id,
        amount: signedAmount,
        reason,
        payment_method: creditDraft.mode === "payment" ? paymentMethod : undefined,
      });
      setCreditDraft(null);
      await refresh();
      showToast("Credito actualizado");
    } catch (error) {
      showToast(String(error));
    }
  };

  return (
    <section className="admin-panel user-admin-grid">
      <form className="user-form" onSubmit={save}>
        <div>
          <h2>Clientes y credito</h2>
          <p>Telefono, limite y saldo.</p>
        </div>
        <label>Nombre<input value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} /></label>
        <label>Telefono<input value={form.phone} onChange={(event) => setForm({ ...form, phone: event.target.value })} /></label>
        <label>Email<input value={form.email} onChange={(event) => setForm({ ...form, email: event.target.value })} placeholder="opcional" /></label>
        <label>Limite credito<input type="number" value={form.credit_limit === 0 ? "" : form.credit_limit} onFocus={selectNumericInput} onChange={(event) => setForm({ ...form, credit_limit: Number(event.target.value) })} /></label>
        <div className="form-button-row">
          {editing && <button className="ghost-button" type="button" onClick={resetForm}>Nuevo cliente</button>}
          <button className="primary-button" type="submit">{editing ? "Actualizar cliente" : "Guardar cliente"}</button>
        </div>
      </form>
      <div className="users-list">
        <div className="module-toolbar slim"><div><h2>Clientes</h2><p>{customers.length} registrados</p></div></div>
        <div className="customer-row customer-head">
          <span>Cliente</span>
          <span>Saldo</span>
          <span>Limite</span>
          <span>Disponible</span>
          <span>Acciones</span>
        </div>
        {customers.map((customer) => (
          <div className="customer-row" key={customer.id}>
            <div>
              <strong>{customer.name}</strong>
              <span>{customer.phone || "Sin telefono"}{customer.email ? ` · ${customer.email}` : ""}</span>
            </div>
            <strong>{money(customer.balance)}</strong>
            <span>{money(customer.credit_limit)}</span>
            <span>{money(Math.max(0, customer.credit_limit - customer.balance))}</span>
            <div className="customer-actions">
            <button className="ghost-button" type="button" onClick={() => setForm({
              id: customer.id,
              name: customer.name,
              phone: customer.phone ?? "",
              email: customer.email ?? "",
              credit_limit: customer.credit_limit,
            })}>Editar</button>
              <button className="ghost-button" type="button" onClick={() => setCreditDraft({ customer, mode: "charge" })}>Cargo</button>
              <button className="primary-button" type="button" onClick={() => setCreditDraft({ customer, mode: "payment" })}>Abono</button>
              <button className="ghost-button" type="button" disabled={customer.balance <= 0} onClick={() => setCreditDraft({ customer, mode: "payment", initialAmount: customer.balance, reason: "Liquidacion total" })}>Total</button>
            </div>
          </div>
        ))}
      </div>
      {creditDraft && (
        <CustomerCreditModal
          draft={creditDraft}
          onClose={() => setCreditDraft(null)}
          onSave={saveCredit}
        />
      )}
    </section>
  );
}
