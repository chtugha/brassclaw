import asyncio
async def worker(worker_id):
    while True:
        task = await host.await_next_task(worker_id)
        if task is None:
            return
        host.enter_task(task['task_token'])
        await host.resolve_intent(task['task_token'], task['user_input'])
        raise RuntimeError('this fixture never fabricates a successful task result')
async def coordinator():
    while True:
        update = await host.await_pool_update()
        if update is None:
            return
        active = set(update['active_ids'])
        for worker_id in range(update['workers']):
            if worker_id not in active:
                asyncio._replace_sibling(worker_id + 1, worker(worker_id))
                active.add(worker_id)
        for worker_id in update['borrowed_ids']:
            if worker_id not in active:
                asyncio._replace_sibling(worker_id + 1, worker(worker_id))
                active.add(worker_id)
async def main():
    workers = [coordinator()]
    for worker_id in range(worker_count):
        workers.append(worker(worker_id))
    await asyncio.gather(*workers)
asyncio.run(main())
