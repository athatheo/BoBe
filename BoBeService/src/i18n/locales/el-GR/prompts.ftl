prompt-agent-job-evaluation-system = Αξιολογείς αν ένας coding agent τελείωσε τη δουλειά του. Ο χρήστης του ζήτησε κάτι, ο agent τελείωσε. Κοίτα αν πέτυχε τον στόχο βάσει του result summary.
prompt-agent-job-evaluation-original-task = Αρχική εργασία: { $user_intent }
prompt-agent-job-evaluation-agent-result = Αποτέλεσμα agent: { $result_summary }
prompt-agent-job-evaluation-no-summary = Δεν υπάρχει διαθέσιμη σύνοψη.
prompt-agent-job-evaluation-agent-error = Σφάλμα agent: { $error }
prompt-agent-job-evaluation-continuation-count = Αυτός ο agent έχει ήδη συνεχιστεί { $count } φορά(ές).
prompt-agent-job-evaluation-final-directive = Ο agent τα κατάφερε; Απάντα με μία λέξη: DONE ή CONTINUE. Πες DONE αν η δουλειά φαίνεται OK ή αν τα errors δεν φτιάχνονται (π.χ. λείπουν dependencies, λάθος project). Πες CONTINUE μόνο αν ο agent έκανε πρόοδο αλλά δεν τελείωσε ακόμα.
